use libloading::Library;
use serde::Serialize;
use std::{
    ffi::{c_char, c_void, CStr},
    path::PathBuf,
    ptr,
};

#[derive(Serialize)]
pub(crate) struct NdiRuntimeStatus {
    available: bool,
    library: Option<String>,
    error: Option<String>,
}

#[derive(Serialize)]
pub(crate) struct NdiSource {
    name: String,
    url: Option<String>,
}

#[repr(C)]
struct NdiFindCreate {
    show_local_sources: bool,
    p_groups: *const c_char,
    p_extra_ips: *const c_char,
}

#[repr(C)]
struct NdiSourceRaw {
    p_ndi_name: *const c_char,
    p_url_address: *const c_char,
}

type NdiInitialize = unsafe extern "C" fn() -> bool;
type NdiDestroy = unsafe extern "C" fn();
type NdiFindCreateV2 = unsafe extern "C" fn(*const NdiFindCreate) -> *mut c_void;
type NdiFindDestroy = unsafe extern "C" fn(*mut c_void);
type NdiFindWait = unsafe extern "C" fn(*mut c_void, u32) -> bool;
type NdiFindGetSources =
    unsafe extern "C" fn(*mut c_void, *mut u32) -> *const NdiSourceRaw;

fn ndi_candidates() -> Vec<PathBuf> {
    let mut paths = Vec::new();

    if let Ok(dir) = std::env::var("NDI_RUNTIME_DIR_V6") {
        let base = PathBuf::from(dir);

        #[cfg(target_os = "windows")]
        paths.push(base.join("Processing.NDI.Lib.x64.dll"));

        #[cfg(target_os = "macos")]
        paths.push(base.join("libndi.dylib"));
    }

    #[cfg(target_os = "windows")]
    {
        paths.push(PathBuf::from(
            r"C:\Program Files\NDI\NDI 6 Runtime\v6\Processing.NDI.Lib.x64.dll",
        ));
        paths.push(PathBuf::from(
            r"C:\Program Files\NDI\NDI 6 Tools\Runtime\Processing.NDI.Lib.x64.dll",
        ));
        paths.push(PathBuf::from("Processing.NDI.Lib.x64.dll"));
    }

    #[cfg(target_os = "macos")]
    {
        paths.push(PathBuf::from("/usr/local/lib/libndi.dylib"));
        paths.push(PathBuf::from("/Library/NDI/lib/libndi.dylib"));
        paths.push(PathBuf::from(
            "/Library/NDI SDK for Apple/lib/macOS/libndi.dylib",
        ));
        paths.push(PathBuf::from("libndi.dylib"));
    }

    paths
}

fn load_ndi() -> Result<(Library, String), String> {
    let mut last_error = String::from("No NDI runtime candidates found");

    for path in ndi_candidates() {
        let label = path.display().to_string();

        match unsafe { Library::new(&path) } {
            Ok(library) => return Ok((library, label)),
            Err(error) => last_error = format!("{label}: {error}"),
        }
    }

    Err(last_error)
}

#[tauri::command]
pub(crate) fn ndi_runtime_status() -> NdiRuntimeStatus {
    match load_ndi() {
        Ok((_library, path)) => NdiRuntimeStatus {
            available: true,
            library: Some(path),
            error: None,
        },
        Err(error) => NdiRuntimeStatus {
            available: false,
            library: None,
            error: Some(error),
        },
    }
}

#[tauri::command]
pub(crate) fn discover_ndi_sources(
    timeout_ms: Option<u32>,
) -> Result<Vec<NdiSource>, String> {
    let (library, _) = load_ndi()?;

    unsafe {
        let initialize: libloading::Symbol<NdiInitialize> = library
            .get(b"NDIlib_initialize\0")
            .map_err(|error| error.to_string())?;

        let destroy: libloading::Symbol<NdiDestroy> = library
            .get(b"NDIlib_destroy\0")
            .map_err(|error| error.to_string())?;

        let find_create: libloading::Symbol<NdiFindCreateV2> = library
            .get(b"NDIlib_find_create_v2\0")
            .map_err(|error| error.to_string())?;

        let find_destroy: libloading::Symbol<NdiFindDestroy> = library
            .get(b"NDIlib_find_destroy\0")
            .map_err(|error| error.to_string())?;

        let find_wait: libloading::Symbol<NdiFindWait> = library
            .get(b"NDIlib_find_wait_for_sources\0")
            .map_err(|error| error.to_string())?;

        let find_get: libloading::Symbol<NdiFindGetSources> = library
            .get(b"NDIlib_find_get_current_sources\0")
            .map_err(|error| error.to_string())?;

        if !initialize() {
            return Err("NDI runtime refused initialization".into());
        }

        let config = NdiFindCreate {
            show_local_sources: true,
            p_groups: ptr::null(),
            p_extra_ips: ptr::null(),
        };

        let finder = find_create(&config);

        if finder.is_null() {
            destroy();
            return Err("Could not create NDI finder".into());
        }

        let _ = find_wait(finder, timeout_ms.unwrap_or(1000).min(5000));

        let mut source_count = 0u32;
        let source_ptr = find_get(finder, &mut source_count as *mut u32);

        let sources = if source_ptr.is_null() || source_count == 0 {
            Vec::new()
        } else {
            let source_slice =
                std::slice::from_raw_parts(source_ptr, source_count as usize);

            source_slice
                .iter()
                .map(|source| {
                    let name = if source.p_ndi_name.is_null() {
                        "Unnamed NDI Source".to_string()
                    } else {
                        CStr::from_ptr(source.p_ndi_name)
                            .to_string_lossy()
                            .into_owned()
                    };

                    let url = if source.p_url_address.is_null() {
                        None
                    } else {
                        Some(
                            CStr::from_ptr(source.p_url_address)
                                .to_string_lossy()
                                .into_owned(),
                        )
                    };

                    NdiSource { name, url }
                })
                .collect()
        };

        find_destroy(finder);
        destroy();

        Ok(sources)
    }
}

#[cfg(test)]
mod tests {
    use super::ndi_candidates;

    #[test]
    fn ndi_candidate_list_is_not_empty_on_desktop() {
        assert!(!ndi_candidates().is_empty());
    }
}

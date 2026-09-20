# LumaLink

LumaLink is a desktop system-I/O utility for MIDI and NDI. It is intended to become the shared connectivity layer for LumaStudio, LumaRig, LumaViz, DAWs, lighting software, OBS and other production applications.

## v0.1 foundation

- physical MIDI input/output discovery
- live MIDI monitor
- direct MIDI input → output routing
- MIDI panic across all outputs
- CoreMIDI virtual input/output bus creation on macOS
- NDI Runtime detection on macOS and Windows
- NDI LAN source discovery via dynamic loading, so the repository does not need to vendor or link the NDI SDK
- separate MIDI, Routing, Monitor, NDI and Settings surfaces
- macOS DMG and Windows NSIS EXE GitHub Actions builds

## Architecture

The frontend is React/TypeScript. Native transport code is Rust inside Tauri 2. Routing lives behind native commands so future Windows MIDI Services, MIDI 2.0, mapper and background-service modules do not require rewriting the UI.

### MIDI

`midir` supplies cross-platform MIDI 1.0 physical I/O. macOS virtual ports use CoreMIDI through `midir`'s virtual-port API. Windows native virtual devices are intentionally kept behind a separate backend boundary because they belong to Windows MIDI Services rather than WinMM.

### NDI

NDI is dynamically loaded at runtime. LumaLink checks `NDI_RUNTIME_DIR_V6` plus common platform install locations and calls the NDI finder API only when the runtime is present. The app remains buildable without the NDI SDK or NDI runtime installed on the build machine.

## Development

```bash
npm install
npm run icons
npm run tauri dev
```

Build locally:

```bash
npm run icons
npm run tauri build
```

## Test signing

macOS uses Tauri `bundle.macOS.signingIdentity = "-"` for ad-hoc signing of test builds. This avoids the malformed/unsigned app-bundle failure that can make downloaded test DMGs appear damaged. Public production distribution should later use Developer ID signing, notarization and stapling.

Windows test builds are unsigned NSIS installers. Authenticode signing can be added to release jobs later without changing application code.

## Next backend modules

1. Windows MIDI Services app-owned virtual devices and persistent loopback management
2. route graph persistence and cycle detection
3. MIDI transform/mapping stack
4. MIDI clock status and clock-source routing
5. NDI receive preview and NDI router aliases
6. detached background routing service

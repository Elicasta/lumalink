# Installer Validation

LumaLink validates distributable artifacts separately from source compilation.

- macOS test releases build a DMG with ad-hoc app signing.
- the workflow mounts the finished DMG and runs `codesign --verify --deep --strict` against the exact `LumaLink.app` the user will install.
- Windows test releases build an NSIS EXE.
- production Developer ID notarization and Windows Authenticode signing remain separate release credentials.
- CI still checks TypeScript and Rust independently so packaging failures are distinguishable from application compile failures.

Tauri removes its intermediate `.app` after creating a DMG, so verification must target the app inside the mounted DMG rather than the cleaned build directory.

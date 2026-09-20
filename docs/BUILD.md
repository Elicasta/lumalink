# Installer Validation

LumaLink validates distributable artifacts separately from source compilation.

- macOS test releases build a DMG with ad-hoc app signing and run `codesign --verify --deep --strict` before artifact upload.
- Windows test releases build an NSIS EXE.
- production Developer ID notarization and Windows Authenticode signing remain separate release credentials.
- CI still checks TypeScript and Rust independently so packaging failures are distinguishable from application compile failures.

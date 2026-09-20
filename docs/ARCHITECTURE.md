# LumaLink Architecture

LumaLink separates transport code from operator UI so MIDI and NDI can evolve without turning the desktop shell into the routing engine.

## Layers

1. **Desktop UI**: React/TypeScript tabs for device inventory, routing, monitoring, NDI and settings.
2. **Native command layer**: Tauri commands expose typed operations to the UI.
3. **MIDI backend**: physical MIDI discovery, monitor, direct routing and platform virtual-endpoint adapters.
4. **NDI backend**: runtime detection and source discovery through dynamic loading.
5. **Service boundary**: reserved for long-running routes, persistence and startup-at-login behavior.

## Platform policy

- macOS uses CoreMIDI virtual ports.
- Windows physical MIDI works through the cross-platform backend.
- Windows virtual devices will use Windows MIDI Services rather than a custom kernel driver.
- NDI libraries are not required at compile time. The installed runtime is loaded when available.

## Build policy

- every pull request verifies frontend build, Rust check and Rust tests on macOS and Windows.
- test DMGs use ad-hoc macOS signing.
- Windows test installers use NSIS.
- production signing and notarization remain a release concern, not a development-build blocker.

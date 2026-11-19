# Releases and installers

- `npm run app:build` produces the executable and, on Windows, MSI and NSIS installers. Not yet run on Windows.
- `.github/workflows/ci.yml` and `release.yml` are provided as a starting point and have **not** been executed.
- To do: build on Windows and test install/uninstall; choose a licence; code signing (unsigned installers trigger
  SmartScreen); application icon review; automatic update (Tauri updater) if wanted; CH340 driver note in the installer.
- Linux: builds with `libwebkit2gtk-4.1-dev`; for USB ids enable the `udev` feature of `tb-core` (needs `libudev-dev`).

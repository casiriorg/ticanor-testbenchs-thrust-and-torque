# Releases and installers

- `npm run app:build` produces the executable and, on Windows, MSI and NSIS installers. Built and smoke-tested on
  Windows for v0.1.0 (`Ticanor Testbench_0.1.0_x64-setup.exe`, `..._x64_en-US.msi`).
- `.github/workflows/release.yml` builds and attaches those two installers to a draft GitHub release whenever a
  `v*` tag is pushed; `ci.yml` runs the core/UI builds and tests on every push and PR. Exercised for the first time
  by the v0.1.0 tag.
- To do: test install/uninstall; code signing (unsigned installers trigger SmartScreen); application icon review;
  automatic update (Tauri updater) if wanted; CH340 driver note in the installer.
- Linux: builds with `libwebkit2gtk-4.1-dev`; for USB ids enable the `udev` feature of `tb-core` (needs `libudev-dev`).

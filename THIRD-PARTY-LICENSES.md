# Third-party licenses

ThrustBench itself is MIT licensed (see `LICENSE`). This file records what was checked across its
dependencies (Rust crates in `app/`, npm packages in `app/`, and the Python packages in `analysis/`) and
the one case that needs more than "it's MIT too."

## Summary

Everything below is MIT, Apache-2.0, BSD, ISC, Zlib, Unicode-3.0 or another permissive license, or a
crate offered under a choice of licenses where MIT (or an equally permissive option) is one of the
choices, with one exception: a handful of crates are MPL-2.0 only. None of this project's dependencies
are GPL, LGPL or AGPL licensed in a way that applies to this project (a couple of crates offer an
LGPL/GPL option alongside MIT/Apache-2.0; picking the MIT option, as this project does, does not invoke
those terms).

## MPL-2.0 dependencies

The Rust build pulls in these Mozilla Public License 2.0 crates:

- `serialport` (direct dependency, `app/crates/tb-core`, for the serial port to the acquisition board)
- `cssparser`, `cssparser-macros`, `selectors`, `dtoa-short` (transitive, via Tauri's `wry`/`tauri-utils`)
- `option-ext` (transitive, via the `dirs` crate, itself pulled in by Tauri)

MPL-2.0 is file-level ("weak") copyleft: it only requires that *modifications to MPL-covered files
themselves* stay under MPL-2.0 and have their source available. It explicitly allows combining MPL code
with differently-licensed code (including proprietary or permissively-licensed code, such as this
project's MIT code) into a larger work distributed under different terms. This project uses all of the
crates above unmodified, as ordinary Cargo dependencies with their source already public on crates.io, so
no action beyond this notice is required. Anyone who *modifies* one of these crates' own source and
redistributes it must do so under MPL-2.0 for those files.

## Build-time only, not distributed

- `caniuse-lite` (CC BY 4.0): browser compatibility data used by `browserslist` during the frontend build
  (`vite build`). It ships in no build output; only its data is read at build time.

## Python (`analysis/`)

`numpy`, `pandas`, `scipy` and `jupyterlab` are BSD-3-Clause; `matplotlib` uses its own PSF-derived
permissive license. All compatible with MIT.

## Everything else

The remaining ~450 Rust crates and ~90 npm packages resolve to MIT, Apache-2.0 (optionally "WITH
LLVM-exception"), BSD-2/3-Clause, ISC, Zlib, 0BSD, Unlicense, CC0-1.0 or Unicode-3.0, used singly,
dual-licensed ("X OR Y"), or combined ("X AND Y"); always with at least one permissive option compatible
with this project's MIT license. This was checked by reading each package's own `license` field (`cargo
metadata` for Rust, each package's `package.json` for npm) rather than assumed from memory.

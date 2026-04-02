# cadrum

[![GitHub License](https://img.shields.io/github/license/lzpel/cadrum)](https://github.com/lzpel/cadrum/blob/main/LICENSE)
[![Crates.io](https://img.shields.io/crates/v/cadrum.svg?logo=rust)](https://crates.io/crates/cadrum)
[![Docs](https://img.shields.io/badge/docs-lzpel.github.io%2Fcadrum-blue)](https://lzpel.github.io/cadrum)

Rust CAD library powered by [OpenCASCADE](https://dev.opencascade.org/) (OCCT 7.9.3).

<p align="center">
  <img src="figure/chijin.svg" alt="chijin — a drum of Amami Oshima" width="360"/>
</p>

## Usage

More examples with source code are available at [lzpel.github.io/cadrum](https://lzpel.github.io/cadrum).

Add this to your `Cargo.toml`:

```toml
[dependencies]
cadrum = "^0.4"
```

Primitives: box, cylinder, sphere, cone, torus — colored and exported as STEP + SVG. ([`examples/01_primitives.rs`](examples/01_primitives.rs))

```sh
cargo run --example 01_primitives
```

```rust
use cadrum::{Color, Shape, Solid};
use glam::DVec3;

fn main() {
    let example_name = std::path::Path::new(file!()).file_stem().unwrap().to_str().unwrap();

    let box_ = Solid::box_from_corners(DVec3::ZERO, DVec3::new(10.0, 20.0, 30.0))
        .color_paint(Some(Color::from_str("#4a90d9").unwrap()));
    let cylinder = Solid::cylinder(DVec3::new(30.0, 0.0, 0.0), 8.0, DVec3::Z, 30.0)
        .color_paint(Some(Color::from_str("#e67e22").unwrap()));
    let sphere = Solid::sphere(DVec3::new(60.0, 0.0, 15.0), 8.0)
        .color_paint(Some(Color::from_str("#2ecc71").unwrap()));
    let cone = Solid::cone(DVec3::new(90.0, 0.0, 0.0), DVec3::Z, 8.0, 0.0, 30.0)
        .color_paint(Some(Color::from_str("#e74c3c").unwrap()));
    let torus = Solid::torus(DVec3::new(130.0, 0.0, 15.0), DVec3::Z, 12.0, 4.0)
        .color_paint(Some(Color::from_str("#9b59b6").unwrap()));

    let shapes = vec![box_, cylinder, sphere, cone, torus];

    let mut f = std::fs::File::create(format!("{example_name}.step")).expect("failed to create file");
    cadrum::write_step_with_colors(&shapes, &mut f).expect("failed to write STEP");

    let svg = shapes.to_svg(DVec3::new(1.0, 1.0, 1.0), 0.5).expect("failed to export SVG");
    std::fs::write(format!("{example_name}.svg"), svg.as_bytes()).expect("failed to write SVG");
}
```

<p align="center">
  <img src="figure/01_primitives.png" alt="01_primitives" width="360"/>
</p>

Full: Make drum with colors, boolean ops, and SVG export, which is shown at the top of this page. ([`examples/05_chijin.rs`](examples/05_chijin.rs))

```sh
cargo run --example 05_chijin
```

Provides safe, ergonomic wrappers around the OCC C++ kernel for:

- Reading/writing STEP and BRep formats (stream-based, no temp files)
- Constructing primitive shapes (box, cylinder, sphere, cone, torus)
- Boolean operations (union, subtract, intersect)
- Face/edge topology traversal
- Meshing with customizable tolerance
- SVG export with hidden-line removal and face colors (`color` feature)

## Requirements

- A C++17 compiler (GCC, Clang, or MSVC)
- CMake

Tested with GCC 15.2.0 (MinGW-w64) and CMake 3.31.11 on Windows.

## Build

By default, `cargo build` downloads OCCT 7.9.3 source and builds it automatically.
The built library is placed in `target/occt/` and removed by `cargo clean`.

To cache the OCCT build across `cargo clean`, set `OCCT_ROOT` to a persistent directory:

```sh
export OCCT_ROOT=~/occt
cargo build
```

- If `OCCT_ROOT` is set and the directory already contains OCCT libraries, they are linked directly (no rebuild).
- If `OCCT_ROOT` is set but the directory is empty or missing, OCCT is built and installed there.
- To force a rebuild, remove the directory: `rm -rf ~/occt`

## Features

- `color` (default): Colored STEP I/O via XDE (`STEPCAFControl`). Enables `write_step_with_colors`,
  `read_step_with_colors`, and per-face color on `Solid`.
  Colors are preserved through boolean operations and other transformations.

## License

This project is licensed under the MIT License.

Compiled binaries include [OpenCASCADE Technology](https://dev.opencascade.org/) (OCCT),
which is licensed under the [LGPL 2.1](https://dev.opencascade.org/resources/licensing).
Users who distribute applications built with cadrum must comply with the LGPL 2.1 terms.
Since cadrum builds OCCT from source, end users can rebuild and relink OCCT to satisfy this requirement.

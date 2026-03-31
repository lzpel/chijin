# cadrum

[![GitHub License](https://img.shields.io/github/license/lzpel/cadrum)](https://github.com/lzpel/cadrum/blob/main/LICENSE)
[![Crates.io](https://img.shields.io/crates/v/cadrum.svg?logo=rust)](https://crates.io/crates/cadrum)
[![Docs](https://img.shields.io/badge/docs-lzpel.github.io%2Fcadrum-blue)](https://lzpel.github.io/cadrum)

Minimal Rust bindings for [OpenCASCADE](https://dev.opencascade.org/) (OCCT 7.9.3) — a solid modeling kernel used in CAD/CAM software.

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

Minimum: Make box and export as step file. 

```sh
cargo run --example box
```

```rust
use cadrum::Solid;
use glam::DVec3;

fn main() {
    let shape = Solid::box_from_corners(DVec3::ZERO, DVec3::new(10.0, 20.0, 30.0));
    let mut f = std::fs::File::create("box.step").expect("failed to create file");
    cadrum::write_step(&[shape], &mut f).expect("failed to write STEP");
}
```

Full: Make drum with colors, boolean ops, and SVG export, which is shown at the top of this page. ([`examples/chijin.rs`](examples/chijin.rs))

```sh
cargo run --example chijin
```

<details>
<summary>Source code</summary>

```rust
use cadrum::{Boolean, Face, Rgb, Shape, Solid};
use glam::DVec3;
use std::f64::consts::PI;

fn chijin() -> Solid {
    // Body (cylinder): r=15, h=8, centered at origin (z=-4..+4)
    let cylinder: Solid = Solid::cylinder(DVec3::new(0.0, 0.0, -4.0), 15.0, DVec3::Z, 8.0)
        .color_paint(Rgb::from_hex("#999").unwrap());

    // Rim: cross-section polygon revolved 360° around Z,
    // mirrored across z=0 to create rims on both top and bottom.
    let cross_section = Face::from_polygon(&[
        DVec3::new(0.0, 0.0, 5.0),
        DVec3::new(0.0, 15.0, 5.0),
        DVec3::new(0.0, 17.0, 3.0),
        DVec3::new(0.0, 15.0, 4.0),
        DVec3::new(0.0, 0.0, 4.0),
        DVec3::new(0.0, 0.0, 5.0),
    ])
    .unwrap();
    let sheet = cross_section
        .revolve(DVec3::ZERO, DVec3::Z, 2.0 * PI)
        .unwrap()
        .color_paint(Rgb::from_hex("#fff").unwrap());
    let sheets = [sheet.mirrored(DVec3::ZERO, DVec3::Z), sheet];

    // Lacing blocks: 2x8x1, rotated 60° around Z, placed at y=15
    let block_proto =
        Solid::box_from_corners(DVec3::new(-1.0, -4.0, -0.5), DVec3::new(1.0, 4.0, 0.5))
            .rotate(DVec3::ZERO, DVec3::Z, 60.0_f64.to_radians())
            .translate(DVec3::new(0.0, 15.0, 0.0));

    // Lacing holes: thin cylinders through each block
    let hole_proto = Solid::cylinder(
        DVec3::new(-5.0, 16.0, -15.0), 0.7, DVec3::new(10.0, 0.0, 30.0), 30.0,
    );

    // Distribute 20 blocks and holes evenly around Z, each block in a rainbow color
    let n = 20usize;
    let mut blocks: Vec<Solid> = Vec::with_capacity(n);
    let mut holes: Vec<Solid> = Vec::with_capacity(n);
    for i in 0..n {
        let angle = 2.0 * PI * (i as f64) / (n as f64);
        let color = Rgb::from_hsv(i as f32 / n as f32, 1.0, 1.0);
        blocks.push(block_proto.clone().rotate(DVec3::ZERO, DVec3::Z, angle).color_paint(color));
        holes.push(hole_proto.clone().rotate(DVec3::ZERO, DVec3::Z, angle));
    }
    let blocks = blocks.into_iter().map(|v| vec![v])
        .reduce(|a, b| Boolean::union(&a, &b).unwrap().solids).unwrap();
    let holes = holes.into_iter().map(|v| vec![v])
        .reduce(|a, b| Boolean::union(&a, &b).unwrap().solids).unwrap();

    // Assemble with boolean operations: union, subtract, union
    let combined: Vec<Solid> = Boolean::union(&[cylinder], &sheets).unwrap().into();
    let result: Vec<Solid> = Boolean::subtract(&combined, &holes).unwrap().into();
    let result: Vec<Solid> = Boolean::union(&result, &blocks).unwrap().into();
    result.into_iter().next().unwrap()
}

fn main() {
    let result = vec![chijin()];
    std::fs::create_dir_all("out").unwrap();

    let mut f = std::fs::File::create("out/chijin.step").unwrap();
    cadrum::write_step_with_colors(&result, &mut f).unwrap();

    let svg = result.to_svg(DVec3::new(1.0, 1.0, 1.0), 0.5).unwrap();
    let mut f = std::fs::File::create("out/chijin.svg").unwrap();
    std::io::Write::write_all(&mut f, svg.as_bytes()).unwrap();
}
```

</details>

Provides safe, ergonomic wrappers around the OCC C++ kernel for:

- Reading/writing STEP and BRep formats (stream-based, no temp files)
- Constructing primitive shapes (box, cylinder, half-space)
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

## Showcase

<p align="center">
  <a href="https://katachiform.com/out/21">
    <img src="figure/katachiform.png" alt="KatachiForm — Sheet Metal Configurator" width="600"/>
  </a>
</p>

- [**KatachiForm**](https://katachiform.com/out/21) — A cloud-native CAD SaaS integrating `cadrum` with **AWS Lambda** for serverless geometry generation and STEP export.

The library was originally named **chijin**, a hand drum traditional to Amami Oshima, a subtropical island of southern Japan.
It was renamed to **cadrum** (CAD + drum) to better convey its purpose as a CAD library while keeping the drum heritage.
[chijin drum](figure/chijin_real.jpg) looks like the 3d figure at the top of this page.

## License

This project is licensed under the MIT License.

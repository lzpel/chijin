# Chijin

Minimal Rust bindings for OpenCASCADE (OCC 7.9).

Provides safe, ergonomic wrappers around the OCC C++ kernel for:
- Reading/writing STEP and BRep formats (stream-based, no temp files)
- Constructing primitive shapes (box, cylinder, half-space)
- Boolean operations (union, subtract, intersect)
- Face/edge topology traversal
- Meshing with customizable tolerance

## Usage

Add this to your `Cargo.toml`:

```toml
[dependencies]
chijin = "0.1.0"
```

## Features

- `buildin` (default): Download and build OCCT 7.9.3 from source during `cargo build`.
- `OCCT_ROOT`: Use a system-installed OCCT via the `OCCT_ROOT` environment variable.

## License

This project is licensed under the MIT License.

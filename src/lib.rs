//! # Chijin
//!
//! Minimal Rust bindings for OpenCASCADE (OCC 7.8).
//!
//! Provides safe, ergonomic wrappers around the OCC C++ kernel for:
//! - Reading/writing STEP and BRep formats (stream-based, no temp files)
//! - Constructing primitive shapes (box, cylinder, half-space)
//! - Boolean operations (union, subtract, intersect)
//! - Face/edge topology traversal
//! - Meshing with customizable tolerance
//!
//! ## Known Bug Fixes
//!
//! This library addresses all known bugs from the previous binding:
//! - **Bug 1**: `STATUS_HEAP_CORRUPTION` — boolean results are auto-deep-copied
//! - **Bug 2**: `STATUS_ACCESS_VIOLATION` on exit — `STEPControl_Reader` leak in C++ layer
//! - **Bug 3**: Mesh normals off-by-one — correct loop bounds
//! - **Bug 4**: Hardcoded approximation tolerance — now parameterized
//! - **Bug 5**: Translation not propagating — uses `BRepBuilderAPI_Transform`

mod edge;
mod error;
mod face;
mod ffi;
mod iterators;
mod mesh;
mod shape;
mod solid;
pub mod stream;
pub mod utils;

pub use edge::Edge;
pub use error::Error;
pub use face::Face;
pub use iterators::{ApproximationSegmentIterator, EdgeIterator, FaceIterator};
pub use mesh::Mesh;
pub use shape::{BooleanShape, Shape};
pub use solid::Solid;
pub use shape::TShapeId;
#[cfg(feature = "color")]
pub use shape::Rgb;

//! # cadrum
//!
//! Minimal Rust bindings for OpenCASCADE (OCCT 7.9.3).
//!
//! ## Core Types
//! - [`Solid`] — a single solid shape (wraps `TopoDS_Shape` / `TopAbs_SOLID`)
//! - [`Shape`] — trait with operations on `[Solid]` / `Vec<Solid>` (import to use methods)

mod edge;
mod error;
mod face;
mod ffi;
mod io;
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
pub use shape::{Boolean, Shape};
pub use shape::TShapeId;
pub use solid::Solid;
#[cfg(feature = "color")]
pub use shape::Rgb;

// I/O functions
pub use io::{read_step, read_brep_bin, read_brep_text};
pub use io::{write_step, write_brep_bin, write_brep_text};
#[cfg(feature = "color")]
pub use io::{read_step_with_colors, read_brep_color};
#[cfg(feature = "color")]
pub use io::{write_step_with_colors, write_brep_color};

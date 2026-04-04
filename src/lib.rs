//! # cadrum
//!
//! Rust CAD library powered by OpenCASCADE (OCCT 7.9.3).
//!
//! ## Core Types
//! - [`Solid`] — a single solid shape (wraps `TopoDS_Shape` / `TopAbs_SOLID`)
//! - [`SolidTrait`] — backend-independent trait for solid operations

pub mod common;
pub mod traits;
pub mod occt;
#[cfg(feature = "pure")]
pub mod pure;

// Re-export the unified trait
pub use traits::SolidTrait;

// Re-export OCCT types at crate root for backward compatibility
pub use occt::edge::Edge;
pub use occt::face::Face;
pub use occt::iterators::{ApproximationSegmentIterator, EdgeIterator, FaceIterator};
pub use occt::shape::Boolean;
pub use occt::shape::TShapeId;
pub use occt::solid::Solid;

// Re-export common types
pub use common::error::Error;
pub use common::mesh::{EdgeData, Mesh};
#[cfg(feature = "color")]
pub use common::color::Color;

// I/O functions
pub use occt::io::{read_step, read_brep_bin, read_brep_text};
pub use occt::io::{write_step, write_brep_bin, write_brep_text};
#[cfg(feature = "color")]
pub use occt::io::{read_step_with_colors, read_brep_color};
#[cfg(feature = "color")]
pub use occt::io::{write_step_with_colors, write_brep_color};

// Re-export submodules
pub use occt::utils;
pub use occt::stream;

//! # cadrum
//!
//! Rust CAD library powered by OpenCASCADE (OCCT 7.9.3).
//!
//! ## Core Types
//! - [`Solid`] — a single solid shape (wraps `TopoDS_Shape` / `TopAbs_SOLID`)
//! - [`Solid`] has all methods directly (no trait import needed)

pub mod common;
#[cfg(not(feature = "pure"))]
pub mod occt;
#[cfg(feature = "pure")]
pub mod pure;
pub(crate) mod traits;
pub use traits::{is_shape_face, is_tool_face, SolidExt};

// Re-export backend types at crate root
#[cfg(not(feature = "pure"))]
pub use occt::edge::Edge;
#[cfg(not(feature = "pure"))]
pub use occt::face::Face;
#[cfg(not(feature = "pure"))]
use occt::io::Io; // private: used by generated delegation, not exposed to users
#[cfg(not(feature = "pure"))]
pub use occt::solid::Solid;

// Re-export common types
#[cfg(feature = "color")]
pub use common::color::Color;
pub use common::error::Error;
pub use common::mesh::{EdgeData, Mesh};
pub use glam::DVec3;

// Re-export submodules
pub use common::utils;

// Auto-generated inherent method delegations (trait methods → pub fn on concrete types)
include!(concat!(env!("OUT_DIR"), "/generated_delegation.rs"));

//! # cadrum
//!
//! Rust CAD library powered by OpenCASCADE (OCCT 8.0.0-rc5).
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
// `Transform` is intentionally NOT re-exported. It remains reachable only as
// `crate::traits::Transform` for internal use; external callers reach the same
// surface through `Compound` / `Wire` forwarder default methods. See the
// `Transform` doc comment in `traits.rs` for the rationale and the future
// auto-delegation plan.
pub use traits::{BSplineEnd, Compound, ProfileOrient, Wire};

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

// ==================== Boolean metadata helpers ====================
//
// Free functions over the active backend's concrete `Face`. They live here
// (not in `traits.rs`) so the trait layer stays free of backend type names.

/// Check if a face came from the tool (b-side) of a boolean operation.
pub fn is_tool_face(metadata: &[Vec<u64>; 2], face: &Face) -> bool {
	metadata[1].contains(&face.tshape_id())
}

/// Check if a face came from the shape (a-side) of a boolean operation.
pub fn is_shape_face(metadata: &[Vec<u64>; 2], face: &Face) -> bool {
	metadata[0].contains(&face.tshape_id())
}

// Auto-generated inherent method delegations (trait methods → pub fn on concrete types)
include!(concat!(env!("OUT_DIR"), "/generated_delegation.rs"));

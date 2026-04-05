//! # cadrum
//!
//! Rust CAD library powered by OpenCASCADE (OCCT 7.9.3).
//!
//! ## Core Types
//! - [`Solid`] — a single solid shape (wraps `TopoDS_Shape` / `TopAbs_SOLID`)
//! - [`Solid`] has all methods directly (no trait import needed)

pub mod common;
pub(crate) mod traits;
pub mod occt;
#[cfg(feature = "pure")]
pub mod pure;

// Re-export OCCT types at crate root
pub use occt::edge::Edge;
pub use occt::face::Face;
pub use occt::iterators::{ApproximationSegmentIterator, EdgeIterator, FaceIterator};
pub use occt::boolean::Boolean;
pub use occt::solid::Solid;

// Re-export common types
pub use glam::DVec3;
pub use common::error::Error;
pub use common::mesh::{EdgeData, Mesh};
#[cfg(feature = "color")]
pub use common::color::Color;

// I/O functions
pub use occt::io::{read_step, read_brep_binary, read_brep_text};
pub use occt::io::{write_step, write_brep_binary, write_brep_text};

// Re-export submodules
pub use occt::utils;
pub use occt::stream;

// TODO: モック実装。メッシュ結合を適切な方法に置き換える。
pub fn to_svg(solids: &[Solid], direction: glam::DVec3, tolerance: f64) -> Result<String, Error> {
    let mut combined = Mesh {
        vertices: vec![], uvs: vec![], normals: vec![], indices: vec![],
        face_ids: vec![],
        #[cfg(feature = "color")]
        colormap: std::collections::HashMap::new(),
        edges: EdgeData::default(),
    };
    for s in solids {
        let m = s.mesh_with_tolerance(tolerance)?;
        let offset = combined.vertices.len();
        combined.vertices.extend(&m.vertices);
        combined.uvs.extend(&m.uvs);
        combined.normals.extend(&m.normals);
        combined.indices.extend(m.indices.iter().map(|i| i + offset));
        combined.face_ids.extend(&m.face_ids);
        #[cfg(feature = "color")]
        combined.colormap.extend(&m.colormap);
        combined.edges.polylines.extend(m.edges.polylines);
    }
    Ok(combined.to_svg(direction))
}

// Auto-generated inherent method delegations (trait methods → pub fn on concrete types)
include!(concat!(env!("OUT_DIR"), "/generated_delegation.rs"));

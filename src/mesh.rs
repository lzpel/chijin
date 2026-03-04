use glam::{DVec2, DVec3};

/// A triangle mesh produced by [`Shape::mesh_with_tolerance`](crate::Shape::mesh_with_tolerance).
///
/// All vectors have the same length: one entry per vertex.
/// `indices` contains triangle indices (groups of 3).
#[derive(Debug, Clone)]
pub struct Mesh {
    /// Vertex positions in 3D space.
    pub vertices: Vec<DVec3>,
    /// UV coordinates, normalized to [0, 1] per face.
    pub uvs: Vec<DVec2>,
    /// Vertex normals.
    pub normals: Vec<DVec3>,
    /// Triangle indices (groups of 3, referencing into `vertices`).
    pub indices: Vec<usize>,
    /// Per-triangle face ID (`TopoDS_TShape*` address). Length equals `indices.len() / 3`.
    pub face_ids: Vec<u64>,
}

use crate::traits::EdgeTrait;
use glam::DVec3;

/// An edge in the pure Rust backend.
///
/// Stores a polyline approximation of the edge.
#[derive(Debug, Clone)]
pub struct Edge {
	pub(crate) points: Vec<DVec3>,
}

impl EdgeTrait for Edge {
	fn approximation_segments(&self, _tolerance: f64) -> Vec<DVec3> {
		self.points.clone()
	}
}

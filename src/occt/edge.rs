use super::ffi;
use super::iterators::ApproximationSegmentIterator;
use crate::traits::EdgeTrait;
use glam::DVec3;

/// An edge topology shape.
pub struct Edge {
	pub(crate) inner: cxx::UniquePtr<ffi::TopoDS_Edge>,
}

impl EdgeTrait for Edge {
	fn approximation_segments(&self, tolerance: f64) -> Vec<DVec3> {
		let approx = ffi::edge_approximation_segments(&self.inner, tolerance);
		ApproximationSegmentIterator::new(approx).collect()
	}
}

impl Edge {
	/// Create an Edge wrapping a `TopoDS_Edge`.
	pub(crate) fn new(inner: cxx::UniquePtr<ffi::TopoDS_Edge>) -> Self {
		Edge { inner }
	}

	/// Get the approximation segments (polyline points) of this edge as an iterator.
	///
	/// `tolerance` controls both the angular deflection (radians) and the
	/// chord deflection (model units) of the approximation. Smaller values
	/// produce more points (finer approximation).
	pub fn approximation_segments_iter(&self, tolerance: f64) -> ApproximationSegmentIterator {
		let approx = ffi::edge_approximation_segments(&self.inner, tolerance);
		ApproximationSegmentIterator::new(approx)
	}

	/// Get the approximation segments with independent angular and chord deflection.
	///
	/// - `angular`: maximum angular deflection in radians between consecutive
	///   tangent directions. Controls how well curves are followed angularly.
	/// - `chord`: maximum chord deflection in model units (straight-line error
	///   between the polyline and the true curve). Controls absolute accuracy.
	///
	/// Use this when you need finer control than the single-tolerance
	/// [`approximation_segments`](Self::approximation_segments) API allows.
	pub fn approximation_segments_ex(
		&self,
		angular: f64,
		chord: f64,
	) -> ApproximationSegmentIterator {
		let approx = ffi::edge_approximation_segments_ex(&self.inner, angular, chord);
		ApproximationSegmentIterator::new(approx)
	}
}

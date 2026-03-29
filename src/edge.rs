use crate::ffi;
use crate::iterators::ApproximationSegmentIterator;

/// An edge topology shape.
pub struct Edge {
	pub(crate) inner: cxx::UniquePtr<ffi::TopoDS_Edge>,
}

impl Edge {
	/// Create an Edge wrapping a `TopoDS_Edge`.
	pub(crate) fn new(inner: cxx::UniquePtr<ffi::TopoDS_Edge>) -> Self {
		Edge { inner }
	}

	/// Get the approximation segments (polyline points) of this edge.
	///
	/// `tolerance` controls both the angular deflection (radians) and the
	/// chord deflection (model units) of the approximation. Smaller values
	/// produce more points (finer approximation).
	///
	/// # Bug 4 fix
	/// In the previous binding, tolerance was hardcoded to 0.1 for both
	/// angular and chord deflection. Now it is parameterized.
	pub fn approximation_segments(&self, tolerance: f64) -> ApproximationSegmentIterator {
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
	///
	/// # Example
	/// ```no_run
	/// # use cadrum::{Shape, Solid};
	/// # let shape: Vec<Solid> = vec![Solid::box_from_corners(
	/// #     glam::DVec3::ZERO, glam::DVec3::new(10.0, 10.0, 10.0))];
	/// # let edge = shape.edges().next().unwrap();
	/// // Fine angular sampling (0.01 rad ≈ 0.57°), coarser chord (1.0 mm)
	/// edge.approximation_segments_ex(0.01, 1.0);
	/// ```
	pub fn approximation_segments_ex(
		&self,
		angular: f64,
		chord: f64,
	) -> ApproximationSegmentIterator {
		let approx = ffi::edge_approximation_segments_ex(&self.inner, angular, chord);
		ApproximationSegmentIterator::new(approx)
	}
}

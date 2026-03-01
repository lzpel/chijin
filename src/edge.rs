use crate::ffi;
use crate::iterators::ApproximationSegmentIterator;
use glam::DVec3;

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
}

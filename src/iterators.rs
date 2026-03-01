use crate::edge::Edge;
use crate::face::Face;
use crate::ffi;
use glam::DVec3;

/// Iterator over faces of a shape.
///
/// Wraps `TopExp_Explorer` with `TopAbs_FACE`.
pub struct FaceIterator {
    explorer: cxx::UniquePtr<ffi::TopExp_Explorer>,
}

impl FaceIterator {
    pub(crate) fn new(explorer: cxx::UniquePtr<ffi::TopExp_Explorer>) -> Self {
        FaceIterator { explorer }
    }
}

impl Iterator for FaceIterator {
    type Item = Face;

    fn next(&mut self) -> Option<Face> {
        if !ffi::explorer_more(&self.explorer) {
            return None;
        }
        let face = ffi::explorer_current_face(&self.explorer);
        ffi::explorer_next(self.explorer.pin_mut());
        Some(Face::new(face))
    }
}

/// Iterator over edges of a shape.
///
/// Wraps `TopExp_Explorer` with `TopAbs_EDGE`.
pub struct EdgeIterator {
    explorer: cxx::UniquePtr<ffi::TopExp_Explorer>,
}

impl EdgeIterator {
    pub(crate) fn new(explorer: cxx::UniquePtr<ffi::TopExp_Explorer>) -> Self {
        EdgeIterator { explorer }
    }
}

impl Iterator for EdgeIterator {
    type Item = Edge;

    fn next(&mut self) -> Option<Edge> {
        if !ffi::explorer_more(&self.explorer) {
            return None;
        }
        let edge = ffi::explorer_current_edge(&self.explorer);
        ffi::explorer_next(self.explorer.pin_mut());
        Some(Edge::new(edge))
    }
}

/// Iterator over approximation points of an edge.
///
/// Each item is a `DVec3` point on the polyline approximation.
pub struct ApproximationSegmentIterator {
    coords: Vec<f64>,
    count: usize,
    index: usize,
}

impl ApproximationSegmentIterator {
    pub(crate) fn new(approx: ffi::ApproxPoints) -> Self {
        ApproximationSegmentIterator {
            coords: approx.coords,
            count: approx.count as usize,
            index: 0,
        }
    }
}

impl Iterator for ApproximationSegmentIterator {
    type Item = DVec3;

    fn next(&mut self) -> Option<DVec3> {
        if self.index >= self.count {
            return None;
        }
        let base = self.index * 3;
        let x = self.coords[base];
        let y = self.coords[base + 1];
        let z = self.coords[base + 2];
        self.index += 1;
        Some(DVec3::new(x, y, z))
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = self.count - self.index;
        (remaining, Some(remaining))
    }
}

impl ExactSizeIterator for ApproximationSegmentIterator {}

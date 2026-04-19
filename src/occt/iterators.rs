use super::ffi;
use glam::DVec3;

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
		ApproximationSegmentIterator { coords: approx.coords, count: approx.count as usize, index: 0 }
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

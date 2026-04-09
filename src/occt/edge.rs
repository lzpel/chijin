use super::ffi;
use super::iterators::ApproximationSegmentIterator;
use crate::common::error::Error;
use crate::traits::{EdgeExt, EdgeStruct, Transform};
use glam::DVec3;

/// An edge topology shape.
pub struct Edge {
	pub(crate) inner: cxx::UniquePtr<ffi::TopoDS_Edge>,
}

impl Edge {
	/// Wrap a FFI-returned `TopoDS_Edge` into `Result<Edge, Error>`, checking
	/// for null. This is the **only** constructor for `Edge` from FFI: all
	/// call sites must go through this function so that no null `TopoDS_Edge`
	/// can silently enter the Rust side.
	///
	/// For paths where null is impossible by construction (Clone, Transform,
	/// iterators — all of which wrap an already-valid edge), callers use
	/// `.expect("...")` with a descriptive message; the panic is unreachable
	/// in practice but serves as a defensive marker.
	pub(crate) fn try_from_ffi(inner: cxx::UniquePtr<ffi::TopoDS_Edge>, msg: String) -> Result<Self, Error> {
		if inner.is_null() {
			Err(Error::InvalidEdge(msg))
		} else {
			Ok(Edge { inner })
		}
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
	pub fn approximation_segments_ex(&self, angular: f64, chord: f64) -> ApproximationSegmentIterator {
		let approx = ffi::edge_approximation_segments_ex(&self.inner, angular, chord);
		ApproximationSegmentIterator::new(approx)
	}
}

impl Clone for Edge {
	fn clone(&self) -> Self {
		Edge::try_from_ffi(ffi::deep_copy_edge(&self.inner), "Edge::clone: deep_copy_edge returned null".into())
			.expect("Edge::clone: unexpected null from deep_copy_edge (this is a bug)")
	}
}

impl EdgeStruct for Edge {
	fn helix(radius: f64, pitch: f64, height: f64, axis: DVec3, x_ref: DVec3) -> Result<Self, Error> {
		let inner = ffi::make_helix_edge(axis.x, axis.y, axis.z, x_ref.x, x_ref.y, x_ref.z, radius, pitch, height);
		Edge::try_from_ffi(inner, format!("helix: degenerate params (radius={radius}, pitch={pitch}, height={height}, axis={axis:?}, x_ref={x_ref:?})"))
	}

	fn polygon(points: impl IntoIterator<Item = DVec3>) -> Result<Vec<Self>, Error> {
		let coords: Vec<f64> = points.into_iter().flat_map(|p| [p.x, p.y, p.z]).collect();
		let cxx_vec = ffi::make_polygon_edges(&coords);
		// C++ 側は失敗時に空ベクタを返す (null ではない)。点数不足や
		// OCCT の MakePolygon 失敗で empty になるので、それを InvalidEdge に変換。
		if cxx_vec.is_empty() {
			return Err(Error::InvalidEdge(format!(
				"polygon: construction failed (point count = {}, need ≥ 3 non-degenerate)",
				coords.len() / 3
			)));
		}
		// CxxVector<TopoDS_Edge> → Vec<Edge>: pull each element out into a
		// UniquePtr<TopoDS_Edge> via deep_copy_edge so we own the topology.
		// deep_copy_edge は既に有効な edge の複製なので null にはならない想定、
		// 万一返った場合は InvalidEdge で failfast する。
		cxx_vec
			.iter()
			.map(|e| {
				Edge::try_from_ffi(
					ffi::deep_copy_edge(e),
					"polygon: deep_copy_edge returned null".into(),
				)
			})
			.collect()
	}

	fn circle(radius: f64, axis: DVec3) -> Result<Self, Error> {
		let inner = ffi::make_circle_edge(axis.x, axis.y, axis.z, radius);
		Edge::try_from_ffi(inner, format!("circle: invalid params (radius={radius}, axis={axis:?})"))
	}

	fn line(a: DVec3, b: DVec3) -> Result<Self, Error> {
		let inner = ffi::make_line_edge(a.x, a.y, a.z, b.x, b.y, b.z);
		Edge::try_from_ffi(inner, format!("line: zero-length segment (a={a:?}, b={b:?})"))
	}

	fn arc_3pts(start: DVec3, mid: DVec3, end: DVec3) -> Result<Self, Error> {
		let inner = ffi::make_arc_edge(start.x, start.y, start.z, mid.x, mid.y, mid.z, end.x, end.y, end.z);
		Edge::try_from_ffi(inner, format!("arc_3pts: collinear or degenerate points (start={start:?}, mid={mid:?}, end={end:?})"))
	}
}

impl EdgeExt for Edge {
	type Elem = Edge;

	fn start_point(&self) -> DVec3 {
		let mut x = 0.0;
		let mut y = 0.0;
		let mut z = 0.0;
		ffi::edge_start_point(&self.inner, &mut x, &mut y, &mut z);
		DVec3::new(x, y, z)
	}

	fn start_tangent(&self) -> DVec3 {
		let mut x = 0.0;
		let mut y = 0.0;
		let mut z = 0.0;
		ffi::edge_start_tangent(&self.inner, &mut x, &mut y, &mut z);
		DVec3::new(x, y, z)
	}

	fn is_closed(&self) -> bool {
		ffi::edge_is_closed(&self.inner)
	}

	fn approximation_segments(&self, tolerance: f64) -> Vec<DVec3> {
		let approx = ffi::edge_approximation_segments(&self.inner, tolerance);
		ApproximationSegmentIterator::new(approx).collect()
	}
}

// Transform は trait 要件で `-> Self` を返すため Result にできない。
// 有効な edge に対するアフィン変換は原理的に失敗しない (OCCT 側でも null を
// 返す経路はない) ので、万一 null が返った場合は expect() で failfast する。
impl Transform for Edge {
	fn translate(self, t: DVec3) -> Self {
		Edge::try_from_ffi(ffi::translate_edge(&self.inner, t.x, t.y, t.z), "Edge::translate: null from FFI".into())
			.expect("Edge::translate: unexpected null from translate_edge (this is a bug)")
	}

	fn rotate(self, axis_origin: DVec3, axis_direction: DVec3, angle: f64) -> Self {
		Edge::try_from_ffi(
			ffi::rotate_edge(&self.inner, axis_origin.x, axis_origin.y, axis_origin.z, axis_direction.x, axis_direction.y, axis_direction.z, angle),
			"Edge::rotate: null from FFI".into(),
		)
		.expect("Edge::rotate: unexpected null from rotate_edge (this is a bug)")
	}

	fn scale(self, center: DVec3, factor: f64) -> Self {
		Edge::try_from_ffi(ffi::scale_edge(&self.inner, center.x, center.y, center.z, factor), "Edge::scale: null from FFI".into())
			.expect("Edge::scale: unexpected null from scale_edge (this is a bug)")
	}

	fn mirror(self, plane_origin: DVec3, plane_normal: DVec3) -> Self {
		Edge::try_from_ffi(
			ffi::mirror_edge(&self.inner, plane_origin.x, plane_origin.y, plane_origin.z, plane_normal.x, plane_normal.y, plane_normal.z),
			"Edge::mirror: null from FFI".into(),
		)
		.expect("Edge::mirror: unexpected null from mirror_edge (this is a bug)")
	}
}

use super::compound::CompoundShape;
use super::edge::Edge;
use super::face::Face;
use super::ffi;
use super::iterators::{EdgeIterator, FaceIterator};
use crate::common::error::Error;
use crate::traits::{Compound, ProfileOrient, SolidStruct, Transform};
use glam::DVec3;
use std::sync::Mutex;

// OCCT の BRepOffsetAPI_ThruSections は内部で global state (おそらく
// BSplCLib のキャッシュや GeomFill_AppSurf の作業バッファ) を使うため、
// 複数スレッドから同時に呼び出すと heap corruption を起こす。
// 並列テスト実行下で再現する症状で、loft 呼び出し全体を Mutex で
// serialize すれば回避できる。性能劣化はあるが loft は重い操作なので
// ロック粒度の粗さは現実的に問題にならない。
static LOFT_LOCK: Mutex<()> = Mutex::new(());

/// Encode `ProfileOrient` into FFI arguments: (kind, ux, uy, uz, aux_spine_edges).
fn encode_orient(orient: ProfileOrient) -> (u32, f64, f64, f64, cxx::UniquePtr<cxx::CxxVector<ffi::TopoDS_Edge>>) {
	let mut aux_vec = ffi::edge_vec_new();
	let (kind, ux, uy, uz) = match orient {
		ProfileOrient::Fixed => (0u32, 0.0, 0.0, 0.0),
		ProfileOrient::Torsion => (1u32, 0.0, 0.0, 0.0),
		ProfileOrient::Up(v) => (2u32, v.x, v.y, v.z),
		ProfileOrient::Auxiliary(edges) => {
			for e in edges {
				ffi::edge_vec_push(aux_vec.pin_mut(), &e.inner);
			}
			(3u32, 0.0, 0.0, 0.0)
		}
	};
	(kind, ux, uy, uz, aux_vec)
}

#[cfg(feature = "color")]
fn remap_colormap_by_order(old_inner: &ffi::TopoDS_Shape, new_inner: &ffi::TopoDS_Shape, old_colormap: &std::collections::HashMap<u64, crate::common::color::Color>) -> std::collections::HashMap<u64, crate::common::color::Color> {
	let mut colormap = std::collections::HashMap::new();
	for (old_face, new_face) in FaceIterator::new(ffi::explore_faces(old_inner)).zip(FaceIterator::new(ffi::explore_faces(new_inner))) {
		if let Some(&color) = old_colormap.get(&old_face.tshape_id()) {
			colormap.insert(new_face.tshape_id(), color);
		}
	}
	colormap
}

/// A single solid topology shape wrapping a `TopoDS_Shape` guaranteed to be `TopAbs_SOLID`.
///
/// `inner` is private to prevent external mutation that could break the solid invariant.
/// Use the provided methods to query and transform the solid.
pub struct Solid {
	inner: cxx::UniquePtr<ffi::TopoDS_Shape>,
	#[cfg(feature = "color")]
	colormap: std::collections::HashMap<u64, crate::common::color::Color>,
}

impl Solid {
	/// Create a `Solid` from a `TopoDS_Shape`.
	///
	/// # Panics
	/// Panics if `inner` is not `TopAbs_SOLID` (and not null).
	pub(crate) fn new(inner: cxx::UniquePtr<ffi::TopoDS_Shape>, #[cfg(feature = "color")] colormap: std::collections::HashMap<u64, crate::common::color::Color>) -> Self {
		debug_assert!(ffi::shape_is_null(&inner) || ffi::shape_is_solid(&inner), "Solid::new called with a non-SOLID shape");
		Solid {
			inner,
			#[cfg(feature = "color")]
			colormap,
		}
	}

	// ==================== Internal accessors ====================

	/// Borrow the underlying `TopoDS_Shape` (crate-internal only).
	pub(crate) fn inner(&self) -> &ffi::TopoDS_Shape {
		&self.inner
	}

	/// Return the underlying `TopoDS_TShape*` address as a `u64`.
	pub fn tshape_id(&self) -> u64 {
		ffi::shape_tshape_id(&self.inner)
	}

	// ==================== Color accessors ====================

	/// Read-only access to the per-face colormap.
	#[cfg(feature = "color")]
	pub fn colormap(&self) -> &std::collections::HashMap<u64, crate::common::color::Color> {
		&self.colormap
	}

	/// Mutable access to the per-face colormap.
	#[cfg(feature = "color")]
	pub fn colormap_mut(&mut self) -> &mut std::collections::HashMap<u64, crate::common::color::Color> {
		&mut self.colormap
	}

	// ==================== Constructors ====================

	/// Returns `true` if this solid wraps a null shape.
	pub fn is_null(&self) -> bool {
		ffi::shape_is_null(&self.inner)
	}

	// ==================== Iterators ====================

	/// Get a face iterator over this solid.
	pub fn face_iter(&self) -> FaceIterator {
		FaceIterator::new(ffi::explore_faces(&self.inner))
	}

	/// Get an edge iterator over this solid.
	pub fn edge_iter(&self) -> EdgeIterator {
		EdgeIterator::new(ffi::explore_edges(&self.inner))
	}
}

impl SolidStruct for Solid {
	type Edge = Edge;
	type Face = Face;

	// ==================== Constructors ====================

	fn cube(x: f64, y: f64, z: f64) -> Solid {
		let inner = ffi::make_box(0.0, 0.0, 0.0, x, y, z);
		Solid::new(
			inner,
			#[cfg(feature = "color")]
			std::collections::HashMap::new(),
		)
	}

	fn cylinder(r: f64, axis: DVec3, h: f64) -> Solid {
		let inner = ffi::make_cylinder(0.0, 0.0, 0.0, axis.x, axis.y, axis.z, r, h);
		Solid::new(
			inner,
			#[cfg(feature = "color")]
			std::collections::HashMap::new(),
		)
	}

	fn sphere(radius: f64) -> Solid {
		let inner = ffi::make_sphere(0.0, 0.0, 0.0, radius);
		Solid::new(
			inner,
			#[cfg(feature = "color")]
			std::collections::HashMap::new(),
		)
	}

	fn cone(r1: f64, r2: f64, axis: DVec3, h: f64) -> Solid {
		let inner = ffi::make_cone(0.0, 0.0, 0.0, axis.x, axis.y, axis.z, r1, r2, h);
		Solid::new(
			inner,
			#[cfg(feature = "color")]
			std::collections::HashMap::new(),
		)
	}

	fn torus(r1: f64, r2: f64, axis: DVec3) -> Solid {
		let inner = ffi::make_torus(0.0, 0.0, 0.0, axis.x, axis.y, axis.z, r1, r2);
		Solid::new(
			inner,
			#[cfg(feature = "color")]
			std::collections::HashMap::new(),
		)
	}

	fn half_space(plane_origin: DVec3, plane_normal: DVec3) -> Solid {
		let inner = ffi::make_half_space(plane_origin.x, plane_origin.y, plane_origin.z, plane_normal.x, plane_normal.y, plane_normal.z);
		Solid::new(
			inner,
			#[cfg(feature = "color")]
			std::collections::HashMap::new(),
		)
	}

	// ==================== Topology ====================

	fn iter_edges(&self) -> EdgeIterator {
		EdgeIterator::new(ffi::explore_edges(&self.inner))
	}

	// ==================== Extrude ====================

	fn extrude<'a>(profile: impl IntoIterator<Item = &'a Edge>, dir: DVec3) -> Result<Self, Error> {
		let mut profile_vec = ffi::edge_vec_new();
		for e in profile {
			ffi::edge_vec_push(profile_vec.pin_mut(), &e.inner);
		}
		let shape = ffi::make_extrude(&profile_vec, dir.x, dir.y, dir.z);
		if shape.is_null() {
			return Err(Error::ExtrudeFailed);
		}
		Ok(Solid::new(
			shape,
			#[cfg(feature = "color")]
			std::collections::HashMap::new(),
		))
	}

	// ==================== Sweep ====================

	fn sweep<'a, 'b, 'c>(profile: impl IntoIterator<Item = &'a Edge>, spine: impl IntoIterator<Item = &'b Edge>, orient: ProfileOrient<'c>) -> Result<Self, Error> {
		let mut profile_vec = ffi::edge_vec_new();
		for e in profile {
			ffi::edge_vec_push(profile_vec.pin_mut(), &e.inner);
		}
		let mut spine_vec = ffi::edge_vec_new();
		for e in spine {
			ffi::edge_vec_push(spine_vec.pin_mut(), &e.inner);
		}
		let (kind, ux, uy, uz, aux_vec) = encode_orient(orient);
		let shape = ffi::make_pipe_shell(&profile_vec, &spine_vec, kind, ux, uy, uz, &aux_vec);
		if shape.is_null() {
			return Err(Error::SweepFailed);
		}
		Ok(Solid::new(
			shape,
			#[cfg(feature = "color")]
			std::collections::HashMap::new(),
		))
	}

	// ==================== Loft ====================

	fn loft<'a, S, I>(sections: S) -> Result<Self, Error> where S: IntoIterator<Item = I>, I: IntoIterator<Item = &'a Edge>, Edge: 'a {
		let _guard = LOFT_LOCK.lock().unwrap_or_else(|e| e.into_inner());

		let mut all_edges = ffi::edge_vec_new();
		let mut section_count = 0usize;

		for sec in sections {
			if section_count > 0 {
				ffi::edge_vec_push_null(all_edges.pin_mut());
			}
			let mut count = 0u32;
			for edge in sec {
				ffi::edge_vec_push(all_edges.pin_mut(), &edge.inner);
				count += 1;
			}
			if count == 0 {
				return Err(Error::LoftFailed(format!(
					"loft: section {} is empty (each section must contain ≥1 edge)",
					section_count
				)));
			}
			section_count += 1;
		}

		if section_count < 2 {
			return Err(Error::LoftFailed(format!(
				"loft: need ≥2 sections, got {} (a single section has no thickness to skin across)",
				section_count
			)));
		}

		let shape = ffi::make_loft(&all_edges);
		if shape.is_null() {
			return Err(Error::LoftFailed(format!(
				"loft: OCCT BRepOffsetAPI_ThruSections failed (sections={}). \
				 Check that each section forms a valid closed wire and sections are not coplanar.",
				section_count
			)));
		}
		Ok(Solid::new(
			shape,
			#[cfg(feature = "color")]
			std::collections::HashMap::new(),
		))
	}

	// ==================== Bspline ====================

	fn bspline<const M: usize, const N: usize>(grid: [[DVec3; N]; M], periodic: bool) -> Result<Self, Error> {
		if M < 2 || N < 3 {
			return Err(Error::BsplineFailed(format!("grid must be at least 2x3 (M={}, N={})", M, N)));
		}

		let mut coords = Vec::with_capacity(3 * M * N);
		for row in &grid {
			for p in row {
				coords.push(p.x);
				coords.push(p.y);
				coords.push(p.z);
			}
		}

		let shape = ffi::make_bspline_solid(&coords, M as u32, N as u32, periodic);
		if shape.is_null() {
			return Err(Error::BsplineFailed(format!("OCCT construction failed (M={}, N={}, periodic={})", M, N, periodic)));
		}
		Ok(Solid::new(
			shape,
			#[cfg(feature = "color")]
			std::collections::HashMap::new(),
		))
	}

	// ==================== Boolean primitives ====================

	fn boolean_union<'a, 'b>(a: impl IntoIterator<Item = &'a Self>, b: impl IntoIterator<Item = &'b Self>) -> Result<(Vec<Self>, [Vec<u64>; 2]), Error> where Self: 'a + 'b {
		Self::boolean_union_impl(a, b)
	}

	fn boolean_subtract<'a, 'b>(a: impl IntoIterator<Item = &'a Self>, b: impl IntoIterator<Item = &'b Self>) -> Result<(Vec<Self>, [Vec<u64>; 2]), Error> where Self: 'a + 'b {
		Self::boolean_subtract_impl(a, b)
	}

	fn boolean_intersect<'a, 'b>(a: impl IntoIterator<Item = &'a Self>, b: impl IntoIterator<Item = &'b Self>) -> Result<(Vec<Self>, [Vec<u64>; 2]), Error> where Self: 'a + 'b {
		Self::boolean_intersect_impl(a, b)
	}
}

// ==================== impl Transform for Solid ====================

impl Transform for Solid {
	fn translate(self, translation: DVec3) -> Self {
		let inner = ffi::translate_shape(&self.inner, translation.x, translation.y, translation.z);
		Solid {
			#[cfg(feature = "color")]
			colormap: self.colormap,
			inner,
		}
	}

	fn rotate(self, axis_origin: DVec3, axis_direction: DVec3, angle: f64) -> Self {
		let inner = ffi::rotate_shape(&self.inner, axis_origin.x, axis_origin.y, axis_origin.z, axis_direction.x, axis_direction.y, axis_direction.z, angle);
		Solid {
			#[cfg(feature = "color")]
			colormap: self.colormap,
			inner,
		}
	}

	// scale/mirror consume self for API consistency, but internally clone the geometry.
	// Unlike translate/rotate which use gp_Trsf + shape.Moved() (preserving TShape),
	// scale/mirror cannot use Moved(): since OCCT Fix 0027457 (v7.6), TopLoc_Location
	// rejects gp_Trsf with scale != 1 or negative determinant, because downstream
	// algorithms (meshing, booleans) break on non-rigid transforms in locations.
	// Therefore BRepBuilderAPI_Transform is required, which rebuilds topology.
	// C++ impl: cpp/wrapper.cpp scale_shape() / mirror_shape()
	// See: https://dev.opencascade.org/content/how-scale-or-mirror-shape
	//      BRepBuilderAPI_Transform.cxx:48-49 (myUseModif branch)

	fn scale(self, center: DVec3, factor: f64) -> Self {
		let inner = ffi::scale_shape(&self.inner, center.x, center.y, center.z, factor);
		#[cfg(feature = "color")]
		let colormap = remap_colormap_by_order(&self.inner, &inner, &self.colormap);
		Solid::new(
			inner,
			#[cfg(feature = "color")]
			colormap,
		)
	}

	fn mirror(self, plane_origin: DVec3, plane_normal: DVec3) -> Self {
		let inner = ffi::mirror_shape(&self.inner, plane_origin.x, plane_origin.y, plane_origin.z, plane_normal.x, plane_normal.y, plane_normal.z);
		#[cfg(feature = "color")]
		let colormap = remap_colormap_by_order(&self.inner, &inner, &self.colormap);
		Solid::new(
			inner,
			#[cfg(feature = "color")]
			colormap,
		)
	}
}

// ==================== impl Compound for Solid ====================
//
// Solid-specific per-element ops (queries / color / boolean wrappers / clean).
// `Vec<Solid>` and `[Solid; N]` impls live in src/traits.rs and delegate to this one.
impl Compound for Solid {
	type Elem = Solid;

	fn clean(&self) -> Result<Self, Error> {
		#[cfg(feature = "color")]
		{
			let r = ffi::clean_shape_full(&self.inner);
			if r.is_null() {
				return Err(Error::CleanFailed);
			}
			let inner = ffi::clean_shape_get(&r);
			if inner.is_null() {
				return Err(Error::CleanFailed);
			}
			let mapping = ffi::clean_shape_mapping(&r);
			let mut colormap = std::collections::HashMap::new();
			for pair in mapping.chunks(2) {
				let new_id = pair[0];
				let old_id = pair[1];
				if let Some(&color) = self.colormap.get(&old_id) {
					colormap.entry(new_id).or_insert(color);
				}
			}
			return Ok(Solid::new(inner, colormap));
		}
		#[cfg(not(feature = "color"))]
		{
			let inner = ffi::clean_shape(&self.inner);
			if inner.is_null() {
				return Err(Error::CleanFailed);
			}
			Ok(Solid::new(inner))
		}
	}

	// ==================== Queries ====================

	fn volume(&self) -> f64 {
		ffi::shape_volume(&self.inner)
	}

	fn shell_count(&self) -> u32 {
		ffi::shape_shell_count(&self.inner)
	}

	fn contains(&self, point: DVec3) -> bool {
		ffi::shape_contains_point(&self.inner, point.x, point.y, point.z)
	}

	fn bounding_box(&self) -> [DVec3; 2] {
		let (mut xmin, mut ymin, mut zmin) = (0.0_f64, 0.0_f64, 0.0_f64);
		let (mut xmax, mut ymax, mut zmax) = (0.0_f64, 0.0_f64, 0.0_f64);
		ffi::shape_bounding_box(&self.inner, &mut xmin, &mut ymin, &mut zmin, &mut xmax, &mut ymax, &mut zmax);
		[DVec3::new(xmin, ymin, zmin), DVec3::new(xmax, ymax, zmax)]
	}

	// ==================== Color ====================

	#[cfg(feature = "color")]
	fn color(self, color: impl Into<crate::common::color::Color>) -> Self {
		let c = color.into();
		let colormap = FaceIterator::new(ffi::explore_faces(&self.inner)).map(|f| (f.tshape_id(), c)).collect();
		Self::new(self.inner, colormap)
	}

	#[cfg(feature = "color")]
	fn color_clear(self) -> Self {
		Self::new(self.inner, std::collections::HashMap::new())
	}

	// ==================== Boolean wrappers ====================

	fn union_with_metadata<'a>(&self, tool: impl IntoIterator<Item = &'a Solid>) -> Result<(Vec<Solid>, [Vec<u64>; 2]), Error> {
		<Solid as SolidStruct>::boolean_union([self], tool)
	}

	fn subtract_with_metadata<'a>(&self, tool: impl IntoIterator<Item = &'a Solid>) -> Result<(Vec<Solid>, [Vec<u64>; 2]), Error> {
		<Solid as SolidStruct>::boolean_subtract([self], tool)
	}

	fn intersect_with_metadata<'a>(&self, tool: impl IntoIterator<Item = &'a Solid>) -> Result<(Vec<Solid>, [Vec<u64>; 2]), Error> {
		<Solid as SolidStruct>::boolean_intersect([self], tool)
	}
}

impl Clone for Solid {
	fn clone(&self) -> Self {
		let inner = ffi::deep_copy(&self.inner);
		#[cfg(feature = "color")]
		let colormap = remap_colormap_by_order(&self.inner, &inner, &self.colormap);
		Solid::new(
			inner,
			#[cfg(feature = "color")]
			colormap,
		)
	}
}

// ==================== Boolean operations ====================

#[cfg(feature = "color")]
fn merge_colormaps(from_a: &[u64], from_b: &[u64], colormap_a: &std::collections::HashMap<u64, crate::common::color::Color>, colormap_b: &std::collections::HashMap<u64, crate::common::color::Color>) -> std::collections::HashMap<u64, crate::common::color::Color> {
	let mut result = std::collections::HashMap::new();
	for pair in from_a.chunks(2) {
		if let Some(&color) = colormap_a.get(&pair[1]) {
			result.insert(pair[0], color);
		}
	}
	for pair in from_b.chunks(2) {
		if let Some(&color) = colormap_b.get(&pair[1]) {
			result.insert(pair[0], color);
		}
	}
	result
}

// `ca` / `cb` carry the source colormaps and are only consulted by the
// `color` feature; the boolean result and metadata are derived purely from
// the FFI BooleanShape, so they go unused without `color`.
#[cfg_attr(not(feature = "color"), allow(unused_variables))]
fn build_boolean_result(r: cxx::UniquePtr<ffi::BooleanShape>, ca: CompoundShape, cb: CompoundShape) -> Result<(Vec<Solid>, [Vec<u64>; 2]), Error> {
	let from_a = ffi::boolean_shape_from_a(&r);
	let from_b = ffi::boolean_shape_from_b(&r);
	let inner = ffi::boolean_shape_shape(&r);

	#[cfg(feature = "color")]
	let colormap = merge_colormaps(&from_a, &from_b, ca.colormap(), cb.colormap());

	let compound = CompoundShape::from_raw(
		inner,
		#[cfg(feature = "color")]
		colormap,
	);

	Ok((compound.decompose(), [from_a, from_b]))
}

// Op kind tags matching the C++ side `boolean_op` switch.
const BOOLEAN_OP_FUSE: u32 = 0;
const BOOLEAN_OP_CUT: u32 = 1;
const BOOLEAN_OP_COMMON: u32 = 2;

impl Solid {
	fn boolean_op_impl<'a, 'b>(a: impl IntoIterator<Item = &'a Solid>, b: impl IntoIterator<Item = &'b Solid>, op_kind: u32) -> Result<(Vec<Solid>, [Vec<u64>; 2]), Error> {
		let ca = CompoundShape::new(a);
		let cb = CompoundShape::new(b);
		let r = ffi::boolean_op(ca.inner(), cb.inner(), op_kind);
		if r.is_null() { return Err(Error::BooleanOperationFailed); }
		build_boolean_result(r, ca, cb)
	}

	pub(crate) fn boolean_union_impl<'a, 'b>(a: impl IntoIterator<Item = &'a Solid>, b: impl IntoIterator<Item = &'b Solid>) -> Result<(Vec<Solid>, [Vec<u64>; 2]), Error> {
		Self::boolean_op_impl(a, b, BOOLEAN_OP_FUSE)
	}

	pub(crate) fn boolean_subtract_impl<'a, 'b>(a: impl IntoIterator<Item = &'a Solid>, b: impl IntoIterator<Item = &'b Solid>) -> Result<(Vec<Solid>, [Vec<u64>; 2]), Error> {
		Self::boolean_op_impl(a, b, BOOLEAN_OP_CUT)
	}

	pub(crate) fn boolean_intersect_impl<'a, 'b>(a: impl IntoIterator<Item = &'a Solid>, b: impl IntoIterator<Item = &'b Solid>) -> Result<(Vec<Solid>, [Vec<u64>; 2]), Error> {
		Self::boolean_op_impl(a, b, BOOLEAN_OP_COMMON)
	}
}

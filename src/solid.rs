use crate::error::Error;
use crate::ffi;
use crate::iterators::{EdgeIterator, FaceIterator};
use crate::mesh::Mesh;
use crate::shape::Shape;
use glam::{DVec2, DVec3};

/// A single solid topology shape wrapping a `TopoDS_Shape` guaranteed to be `TopAbs_SOLID`.
///
/// `inner` is private to prevent external mutation that could break the solid invariant.
/// Use the provided methods to query and transform the solid.
pub struct Solid {
	inner: cxx::UniquePtr<ffi::TopoDS_Shape>,
	#[cfg(feature = "color")]
	colormap: std::collections::HashMap<crate::shape::TShapeId, crate::color::Color>,
}

impl Solid {
	/// Create a `Solid` from a `TopoDS_Shape`.
	///
	/// # Panics
	/// Panics if `inner` is not `TopAbs_SOLID` (and not null).
	pub(crate) fn new(
		inner: cxx::UniquePtr<ffi::TopoDS_Shape>,
		#[cfg(feature = "color")]
		colormap: std::collections::HashMap<crate::shape::TShapeId, crate::color::Color>,
	) -> Self {
		debug_assert!(
			ffi::shape_is_null(&inner) || ffi::shape_is_solid(&inner),
			"Solid::new called with a non-SOLID shape"
		);
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

	/// Return the `TShapeId` (underlying `TopoDS_TShape*` address) of this solid.
	pub fn tshape_id(&self) -> crate::shape::TShapeId {
		crate::shape::TShapeId(ffi::shape_tshape_id(&self.inner))
	}

	// ==================== Color accessors ====================

	/// Read-only access to the per-face colormap.
	#[cfg(feature = "color")]
	pub fn colormap(&self) -> &std::collections::HashMap<crate::shape::TShapeId, crate::color::Color> {
		&self.colormap
	}

	/// Mutable access to the per-face colormap.
	#[cfg(feature = "color")]
	pub fn colormap_mut(&mut self) -> &mut std::collections::HashMap<crate::shape::TShapeId, crate::color::Color> {
		&mut self.colormap
	}

	// ==================== Constructors ====================

	/// Create a half-space solid.
	///
	/// The solid fills the half-space on the side **where the normal points**.
	pub fn half_space(plane_origin: DVec3, plane_normal: DVec3) -> Solid {
		let inner = ffi::make_half_space(
			plane_origin.x, plane_origin.y, plane_origin.z,
			plane_normal.x, plane_normal.y, plane_normal.z,
		);
		Solid::new(
			inner,
			#[cfg(feature = "color")]
			std::collections::HashMap::new(),
		)
	}

	/// Create a box from two opposite corner points.
	///
	/// Named `box_from_corners` rather than `box` because `box` is a reserved keyword in Rust.
	pub fn box_from_corners(corner_1: DVec3, corner_2: DVec3) -> Solid {
		let inner = ffi::make_box(
			corner_1.x, corner_1.y, corner_1.z,
			corner_2.x, corner_2.y, corner_2.z,
		);
		Solid::new(
			inner,
			#[cfg(feature = "color")]
			std::collections::HashMap::new(),
		)
	}

	/// Create a cylinder.
	///
	/// - `p`: center of the base circle
	/// - `r`: radius
	/// - `dir`: axis direction
	/// - `h`: height along the axis
	pub fn cylinder(p: DVec3, r: f64, dir: DVec3, h: f64) -> Solid {
		let inner = ffi::make_cylinder(p.x, p.y, p.z, dir.x, dir.y, dir.z, r, h);
		Solid::new(
			inner,
			#[cfg(feature = "color")]
			std::collections::HashMap::new(),
		)
	}

	/// Create a sphere.
	///
	/// - `center`: center point
	/// - `radius`: radius
	pub fn sphere(center: DVec3, radius: f64) -> Solid {
		let inner = ffi::make_sphere(center.x, center.y, center.z, radius);
		Solid::new(
			inner,
			#[cfg(feature = "color")]
			std::collections::HashMap::new(),
		)
	}

	/// Create a cone.
	///
	/// - `p`: center of the base circle
	/// - `dir`: axis direction
	/// - `r1`: bottom radius
	/// - `r2`: top radius (0 for a pointed tip)
	/// - `h`: height along the axis
	pub fn cone(p: DVec3, dir: DVec3, r1: f64, r2: f64, h: f64) -> Solid {
		let inner = ffi::make_cone(p.x, p.y, p.z, dir.x, dir.y, dir.z, r1, r2, h);
		Solid::new(
			inner,
			#[cfg(feature = "color")]
			std::collections::HashMap::new(),
		)
	}

	/// Create a torus.
	///
	/// - `p`: center point
	/// - `dir`: axis direction (normal to the torus plane)
	/// - `r1`: major radius (center to tube center)
	/// - `r2`: minor radius (tube radius)
	pub fn torus(p: DVec3, dir: DVec3, r1: f64, r2: f64) -> Solid {
		let inner = ffi::make_torus(p.x, p.y, p.z, dir.x, dir.y, dir.z, r1, r2);
		Solid::new(
			inner,
			#[cfg(feature = "color")]
			std::collections::HashMap::new(),
		)
	}

}

impl Shape for Solid {
	// ==================== Transforms ====================

	fn translate(self, translation: DVec3) -> Self {
		let inner = ffi::translate_shape(&self.inner, translation.x, translation.y, translation.z);
		Solid {
			#[cfg(feature = "color")]
			colormap: self.colormap,
			inner,
		}
	}

	fn rotate(self, axis_origin: DVec3, axis_direction: DVec3, angle: f64) -> Self {
		let inner = ffi::rotate_shape(
			&self.inner,
			axis_origin.x, axis_origin.y, axis_origin.z,
			axis_direction.x, axis_direction.y, axis_direction.z,
			angle,
		);
		Solid {
			#[cfg(feature = "color")]
			colormap: self.colormap,
			inner,
		}
	}

	fn scaled(&self, center: DVec3, factor: f64) -> Self {
		let inner = ffi::scale_shape(&self.inner, center.x, center.y, center.z, factor);
		#[cfg(feature = "color")]
		let colormap = crate::shape::remap_colormap_by_order(&self.inner, &inner, &self.colormap);
		Solid::new(inner, #[cfg(feature = "color")] colormap)
	}

	fn mirrored(&self, plane_origin: DVec3, plane_normal: DVec3) -> Self {
		let inner = ffi::mirror_shape(
			&self.inner,
			plane_origin.x, plane_origin.y, plane_origin.z,
			plane_normal.x, plane_normal.y, plane_normal.z,
		);
		#[cfg(feature = "color")]
		let colormap = crate::shape::remap_colormap_by_order(&self.inner, &inner, &self.colormap);
		Solid::new(inner, #[cfg(feature = "color")] colormap)
	}

	// ==================== Clean ====================

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
				let new_id = crate::shape::TShapeId(pair[0]);
				let old_id = crate::shape::TShapeId(pair[1]);
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

	fn is_null(&self) -> bool {
		ffi::shape_is_null(&self.inner)
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

	fn faces(&self) -> FaceIterator {
		FaceIterator::new(ffi::explore_faces(&self.inner))
	}

	fn edges(&self) -> EdgeIterator {
		EdgeIterator::new(ffi::explore_edges(&self.inner))
	}

	// ==================== Mesh ====================

	fn mesh_with_tolerance(&self, tol: f64) -> Result<Mesh, Error> {
		let data = ffi::mesh_shape(&self.inner, tol);
		if !data.success {
			return Err(Error::TriangulationFailed);
		}
		let vertex_count = data.vertices.len() / 3;
		let vertices: Vec<DVec3> = (0..vertex_count)
			.map(|i| DVec3::new(data.vertices[i * 3], data.vertices[i * 3 + 1], data.vertices[i * 3 + 2]))
			.collect();
		let uvs: Vec<DVec2> = (0..vertex_count)
			.map(|i| DVec2::new(data.uvs[i * 2], data.uvs[i * 2 + 1]))
			.collect();
		let normals: Vec<DVec3> = (0..vertex_count)
			.map(|i| DVec3::new(data.normals[i * 3], data.normals[i * 3 + 1], data.normals[i * 3 + 2]))
			.collect();
		let indices: Vec<usize> = data.indices.iter().map(|&i| i as usize).collect();
		let face_ids = data.face_tshape_ids;
		Ok(Mesh { vertices, uvs, normals, indices, face_ids })
	}

	fn to_svg(&self, direction: DVec3, tolerance: f64) -> Result<String, Error> {
		vec![self.clone()].to_svg(direction, tolerance)
	}

	// ==================== Color ====================

	#[cfg(feature = "color")]
	fn color_paint(self, color: Option<crate::color::Color>) -> Self {
		let colormap = match color {
			Some(c) => self.faces().map(|f| (f.tshape_id(), c)).collect(),
			None => std::collections::HashMap::new(),
		};
		Self::new(self.inner, colormap)
	}

	#[cfg(feature = "color")]
	fn color(&self) -> Option<crate::color::Color> {
		let colors: Vec<crate::color::Color> = self
			.faces()
			.filter_map(|f| self.colormap.get(&f.tshape_id()).copied())
			.collect();
		if colors.is_empty() {
			None
		}else{
			Some(crate::color::Color {
				r: colors.iter().map(|c| c.r).sum::<f32>() / colors.len() as f32,
				g: colors.iter().map(|c| c.g).sum::<f32>() / colors.len() as f32,
				b: colors.iter().map(|c| c.b).sum::<f32>() / colors.len() as f32,
			})
		}
	}
}

impl Clone for Solid {
	fn clone(&self) -> Self {
		let inner = ffi::deep_copy(&self.inner);
		#[cfg(feature = "color")]
		let colormap = crate::shape::remap_colormap_by_order(&self.inner, &inner, &self.colormap);
		Solid::new(
			inner,
			#[cfg(feature = "color")]
			colormap,
		)
	}
}

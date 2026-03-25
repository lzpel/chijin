use crate::error::Error;
use crate::ffi;
use crate::iterators::{EdgeIterator, FaceIterator};
use crate::mesh::Mesh;
use glam::{DVec2, DVec3};

/// A single solid topology shape wrapping a `TopoDS_Shape` guaranteed to be `TopAbs_SOLID`.
///
/// `inner` is private to prevent external mutation that could break the solid invariant.
/// Use the provided methods to query and transform the solid.
pub struct Solid {
	inner: cxx::UniquePtr<ffi::TopoDS_Shape>,
	#[cfg(feature = "color")]
	colormap: std::collections::HashMap<crate::shape::TShapeId, crate::shape::Rgb>,
}

impl Solid {
	/// Create a `Solid` from a `TopoDS_Shape`.
	///
	/// # Panics
	/// Panics if `inner` is not `TopAbs_SOLID` (and not null).
	pub(crate) fn new(
		inner: cxx::UniquePtr<ffi::TopoDS_Shape>,
		#[cfg(feature = "color")]
		colormap: std::collections::HashMap<crate::shape::TShapeId, crate::shape::Rgb>,
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

	// ==================== Color accessors ====================

	/// Read-only access to the per-face colormap.
	#[cfg(feature = "color")]
	pub fn colormap(&self) -> &std::collections::HashMap<crate::shape::TShapeId, crate::shape::Rgb> {
		&self.colormap
	}

	/// Mutable access to the per-face colormap.
	#[cfg(feature = "color")]
	pub fn colormap_mut(&mut self) -> &mut std::collections::HashMap<crate::shape::TShapeId, crate::shape::Rgb> {
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

	// ==================== Transforms ====================

	/// Create a new solid translated by the given vector.
	pub fn translated(&self, translation: DVec3) -> Solid {
		let inner = ffi::translate_shape(&self.inner, translation.x, translation.y, translation.z);
		#[cfg(feature = "color")]
		let colormap = crate::shape::remap_colormap_by_order(&self.inner, &inner, &self.colormap);
		Solid::new(
			inner,
			#[cfg(feature = "color")]
			colormap,
		)
	}

	/// Create a new solid rotated around an axis.
	pub fn rotated(&self, axis_origin: DVec3, axis_direction: DVec3, angle: f64) -> Solid {
		let inner = ffi::rotate_shape(
			&self.inner,
			axis_origin.x, axis_origin.y, axis_origin.z,
			axis_direction.x, axis_direction.y, axis_direction.z,
			angle,
		);
		#[cfg(feature = "color")]
		let colormap = crate::shape::remap_colormap_by_order(&self.inner, &inner, &self.colormap);
		Solid::new(
			inner,
			#[cfg(feature = "color")]
			colormap,
		)
	}

	/// Create a new solid uniformly scaled around a center point.
	pub fn scaled(&self, center: DVec3, factor: f64) -> Solid {
		let inner = ffi::scale_shape(&self.inner, center.x, center.y, center.z, factor);
		#[cfg(feature = "color")]
		let colormap = crate::shape::remap_colormap_by_order(&self.inner, &inner, &self.colormap);
		Solid::new(
			inner,
			#[cfg(feature = "color")]
			colormap,
		)
	}

	// ==================== Clean ====================

	/// Clean the solid by unifying same-domain faces, edges, and vertices.
	pub fn clean(&self) -> Result<Solid, Error> {
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

	/// Compute the volume of this solid.
	pub fn volume(&self) -> f64 {
		ffi::shape_volume(&self.inner)
	}

	/// Check if this solid is null.
	pub fn is_null(&self) -> bool {
		ffi::shape_is_null(&self.inner)
	}

	/// Count the number of shells in this solid.
	pub fn shell_count(&self) -> u32 {
		ffi::shape_shell_count(&self.inner)
	}

	/// Check if a point is inside this solid.
	pub fn contains(&self, point: DVec3) -> bool {
		ffi::shape_contains_point(&self.inner, point.x, point.y, point.z)
	}

	/// Iterate over all faces in this solid.
	pub fn faces(&self) -> FaceIterator {
		FaceIterator::new(ffi::explore_faces(&self.inner))
	}

	/// Iterate over all edges in this solid.
	pub fn edges(&self) -> EdgeIterator {
		EdgeIterator::new(ffi::explore_edges(&self.inner))
	}

	// ==================== Mesh ====================

	/// Mesh this solid with the given linear deflection tolerance.
	pub fn mesh_with_tolerance(&self, tol: f64) -> Result<Mesh, Error> {
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

	// ==================== Color ====================

	/// Assign the same color to every face in this solid.
	#[cfg(feature = "color")]
	pub fn paint(&mut self, color: crate::shape::Rgb) {
		let ids: Vec<crate::shape::TShapeId> = self.faces().map(|f| f.tshape_id()).collect();
		for id in ids {
			self.colormap.insert(id, color);
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

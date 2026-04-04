use crate::common::error::Error;
use crate::traits::FaceTrait;
use super::ffi;
use super::solid::Solid;
use glam::DVec3;

impl FaceTrait for Face {
	fn normal_at_center(&self) -> DVec3 {
		let mut nx = 0.0;
		let mut ny = 0.0;
		let mut nz = 0.0;
		ffi::face_normal_at_center(&self.inner, &mut nx, &mut ny, &mut nz);
		DVec3::new(nx, ny, nz)
	}

	fn center_of_mass(&self) -> DVec3 {
		let mut cx = 0.0;
		let mut cy = 0.0;
		let mut cz = 0.0;
		ffi::face_center_of_mass(&self.inner, &mut cx, &mut cy, &mut cz);
		DVec3::new(cx, cy, cz)
	}
}

impl Face {
	/// Create a planar face from a polygon defined by 3D points.
	///
	/// Points must be coplanar and form a non-degenerate polygon (at least 3 points).
	/// The face normal follows the right-hand rule: the normal points toward you
	/// when the points appear counter-clockwise.
	///
	/// Uses `BRepBuilderAPI_MakePolygon` + `BRepBuilderAPI_MakeFace`.
	///
	/// # Errors
	/// Returns [`Error::InvalidPolygon`] if the points are non-planar, degenerate,
	/// or fewer than 3 points are provided.
	pub fn from_polygon(points: &[DVec3]) -> Result<Face, Error> {
		let coords: Vec<f64> = points.iter().flat_map(|p| [p.x, p.y, p.z]).collect();
		let inner = ffi::face_from_polygon(&coords);
		if inner.is_null() {
			return Err(Error::InvalidPolygon);
		}
		Ok(Face::new(inner))
	}
}

/// A face topology shape.
pub struct Face {
	pub(crate) inner: cxx::UniquePtr<ffi::TopoDS_Face>,
}

impl Face {
	/// Create a Face wrapping a `TopoDS_Face`.
	pub(crate) fn new(inner: cxx::UniquePtr<ffi::TopoDS_Face>) -> Self {
		Face { inner }
	}

	/// Return the `TShapeId` (underlying `TopoDS_TShape*` address) of this face.
	///
	/// Use this to look up or set entries in `Shape::colormap`,
	/// or to match faces against [`BooleanShape::new_face_ids`].
	pub fn tshape_id(&self) -> super::shape::TShapeId {
		super::shape::TShapeId(ffi::face_tshape_id(&self.inner))
	}

	/// Get the normal vector at the center of mass of this face.
	///
	/// The center of mass is computed using surface-area-weighted integration,
	/// and the normal is evaluated at that point on the surface.
	pub fn normal_at_center(&self) -> DVec3 {
		let mut nx = 0.0;
		let mut ny = 0.0;
		let mut nz = 0.0;
		ffi::face_normal_at_center(&self.inner, &mut nx, &mut ny, &mut nz);
		DVec3::new(nx, ny, nz)
	}

	/// Get the center of mass of this face.
	///
	/// Computed using `BRepGProp::SurfaceProperties`, which gives the
	/// surface-area-weighted centroid.
	pub fn center_of_mass(&self) -> DVec3 {
		let mut cx = 0.0;
		let mut cy = 0.0;
		let mut cz = 0.0;
		ffi::face_center_of_mass(&self.inner, &mut cx, &mut cy, &mut cz);
		DVec3::new(cx, cy, cz)
	}

	/// Extrude this face along the given direction vector to create a solid.
	///
	/// Uses `BRepPrimAPI_MakePrism`. The result can be converted to a `Shape`
	/// via `Shape::from(solid)`.
	pub fn extrude(&self, dir: DVec3) -> Result<Solid, Error> {
		let shape = ffi::face_extrude(&self.inner, dir.x, dir.y, dir.z);
		if shape.is_null() {
			return Err(Error::ExtrudeFailed);
		}
		Ok(Solid::new(
			shape,
			#[cfg(feature = "color")]
			std::collections::HashMap::new(),
		))
	}

	/// Revolve this face around an axis to create a solid.
	///
	/// Uses `BRepPrimAPI_MakeRevol`. The face's current position defines the
	/// start of the revolution (angle = 0). The result can be converted to a
	/// `Shape` via `Shape::from(solid)`.
	///
	/// - `axis_origin`: a point on the rotation axis
	/// - `axis_direction`: direction of the rotation axis (normalised by OCCT)
	/// - `angle`: rotation angle in radians (`std::f64::consts::TAU` for a full revolution)
	///
	/// # Errors
	/// Returns [`Error::RevolveFailed`] if the operation fails (e.g. the face
	/// crosses the rotation axis, causing self-intersection).
	pub fn revolve(&self, axis_origin: DVec3, axis_direction: DVec3, angle: f64) -> Result<Solid, Error> {
		let shape = ffi::face_revolve(
			&self.inner,
			axis_origin.x, axis_origin.y, axis_origin.z,
			axis_direction.x, axis_direction.y, axis_direction.z,
			angle,
		);
		if shape.is_null() {
			return Err(Error::RevolveFailed);
		}
		Ok(Solid::new(
			shape,
			#[cfg(feature = "color")]
			std::collections::HashMap::new(),
		))
	}

	/// Sweep this face along a helical path to create a solid.
	///
	/// The helix radius is automatically computed as the distance from the
	/// face's centre of mass to the axis. The face is moved to the spine
	/// start by `BRepOffsetAPI_MakePipeShell`.
	///
	/// - `axis_origin`: a point on the helix axis
	/// - `axis_direction`: direction of the helix axis (normalised by OCCT)
	/// - `pitch`: height per revolution
	/// - `turns`: number of full revolutions (e.g. `1.0` for one full turn)
	/// - `align_to_spine`: if `true`, the profile is rotated to be perpendicular
	///   to the spine tangent (pipe-sweep); if `false`, the profile keeps its
	///   original orientation (preserves cross-section shape)
	pub fn helix(
		&self,
		axis_origin: DVec3,
		axis_direction: DVec3,
		pitch: f64,
		turns: f64,
		align_to_spine: bool,
	) -> Result<Solid, Error> {
		let shape = ffi::face_helix(
			&self.inner,
			axis_origin.x, axis_origin.y, axis_origin.z,
			axis_direction.x, axis_direction.y, axis_direction.z,
			pitch, turns, align_to_spine,
		);
		if shape.is_null() {
			return Err(Error::HelixFailed);
		}
		Ok(Solid::new(
			shape,
			#[cfg(feature = "color")]
			std::collections::HashMap::new(),
		))
	}
}

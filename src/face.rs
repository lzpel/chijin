use crate::ffi;
use crate::solid::Solid;
use glam::DVec3;

/// A face topology shape.
pub struct Face {
    pub(crate) inner: cxx::UniquePtr<ffi::TopoDS_Face>,
}

impl Face {
    /// Create a Face wrapping a `TopoDS_Face`.
    pub(crate) fn new(inner: cxx::UniquePtr<ffi::TopoDS_Face>) -> Self {
        Face { inner }
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
    pub fn extrude(&self, dir: DVec3) -> Solid {
        let shape = ffi::face_extrude(&self.inner, dir.x, dir.y, dir.z);
        Solid::new(shape)
    }
}

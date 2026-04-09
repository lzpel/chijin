use super::ffi;

/// A face topology shape.
pub struct Face {
	pub(crate) inner: cxx::UniquePtr<ffi::TopoDS_Face>,
}

impl Face {
	/// Create a Face wrapping a `TopoDS_Face`.
	pub(crate) fn new(inner: cxx::UniquePtr<ffi::TopoDS_Face>) -> Self {
		Face { inner }
	}

	/// Return the underlying `TopoDS_TShape*` address as a `u64`.
	///
	/// Use this to look up or set entries in `Solid::colormap`,
	/// or to match faces against boolean operation results.
	pub fn tshape_id(&self) -> u64 {
		ffi::face_tshape_id(&self.inner)
	}
}

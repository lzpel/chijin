use crate::ffi;

/// A solid topology shape (result of extrusion).
pub struct Solid {
	pub(crate) inner: cxx::UniquePtr<ffi::TopoDS_Shape>,
}

impl Solid {
	/// Create a Solid wrapping a `TopoDS_Shape` (assumed to be a solid).
	pub(crate) fn new(inner: cxx::UniquePtr<ffi::TopoDS_Shape>) -> Self {
		Solid { inner }
	}
}

impl From<Solid> for crate::Shape {
	fn from(solid: Solid) -> crate::Shape {
		crate::Shape { inner: solid.inner }
	}
}

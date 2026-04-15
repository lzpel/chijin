use super::ffi;
use super::solid::Solid;
#[cfg(feature = "color")]
use crate::common::color::Color;

/// A compound shape wrapping multiple solids into a single `TopoDS_Compound`.
///
/// Provides type-safe distinction from individual `Solid` handles.
pub(crate) struct CompoundShape {
	inner: cxx::UniquePtr<ffi::TopoDS_Shape>,
	#[cfg(feature = "color")]
	colormap: std::collections::HashMap<u64, Color>,
}

impl CompoundShape {
	/// Assemble solids into a compound, merging their colormaps.
	pub fn new<'a>(solids: impl IntoIterator<Item = &'a Solid>) -> Self {
		let mut inner = ffi::make_empty();
		#[cfg(feature = "color")]
		let mut colormap = std::collections::HashMap::new();
		for s in solids {
			ffi::compound_add(inner.pin_mut(), s.inner());
			#[cfg(feature = "color")]
			colormap.extend(s.colormap().iter().map(|(&k, &v)| (k, v)));
		}
		CompoundShape {
			inner,
			#[cfg(feature = "color")]
			colormap,
		}
	}

	/// Create a compound from a raw `TopoDS_Shape` (e.g. from I/O or boolean ops).
	pub fn from_raw(inner: cxx::UniquePtr<ffi::TopoDS_Shape>, #[cfg(feature = "color")] colormap: std::collections::HashMap<u64, Color>) -> Self {
		CompoundShape {
			inner,
			#[cfg(feature = "color")]
			colormap,
		}
	}

	/// Borrow the underlying `TopoDS_Shape`.
	pub fn inner(&self) -> &ffi::TopoDS_Shape {
		&self.inner
	}

	/// Borrow the merged colormap.
	#[cfg(feature = "color")]
	pub fn colormap(&self) -> &std::collections::HashMap<u64, Color> {
		&self.colormap
	}

	/// Decompose into individual solids, consuming the compound.
	pub fn decompose(self) -> Vec<Solid> {
		let solid_shapes = ffi::decompose_into_solids(&self.inner);
		solid_shapes
			.iter()
			.map(|s| {
				Solid::new(
					ffi::shallow_copy(s),
					#[cfg(feature = "color")]
					self.colormap.clone(),
				)
			})
			.collect()
	}
}

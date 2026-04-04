use crate::common::error::Error;
use crate::common::mesh::{EdgeData, Mesh};
use super::ffi;
use super::iterators::{FaceIterator};
use super::solid::Solid;
#[cfg(feature = "color")]
pub(crate) use crate::common::color::Color;
use glam::{DVec2, DVec3};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TShapeId(pub u64);


// ==================== Internal helpers ====================

/// Assemble solids into a TopoDS_Compound.
pub(crate) fn to_compound<'a>(solids: impl IntoIterator<Item = &'a Solid>) -> cxx::UniquePtr<ffi::TopoDS_Shape> {
	let mut compound = ffi::make_empty();
	for s in solids {
		ffi::compound_add(compound.pin_mut(), s.inner());
	}
	compound
}

/// Decompose a compound TopoDS_Shape into Vec<Solid>.
pub(crate) fn decompose(
	compound: &ffi::TopoDS_Shape,
	#[cfg(feature = "color")] colormap: &std::collections::HashMap<TShapeId, Color>,
) -> Vec<Solid> {
	let solid_shapes = ffi::decompose_into_solids(compound);
	solid_shapes
		.iter()
		.map(|s| {
			let inner = ffi::shallow_copy(s);
			Solid::new(
				inner,
				#[cfg(feature = "color")]
				colormap.clone(),
			)
		})
		.collect()
}

/// Merge colormaps from all solids.
#[cfg(feature = "color")]
pub(crate) fn merge_all_colormaps<'a>(solids: impl IntoIterator<Item = &'a Solid>) -> std::collections::HashMap<TShapeId, Color> {
	let mut merged = std::collections::HashMap::new();
	for s in solids {
		merged.extend(s.colormap().iter().map(|(&k, &v)| (k, v)));
	}
	merged
}

// ==================== Color helpers ====================

#[cfg(feature = "color")]
pub(crate) fn remap_colormap_by_order(
	old_inner: &ffi::TopoDS_Shape,
	new_inner: &ffi::TopoDS_Shape,
	old_colormap: &std::collections::HashMap<TShapeId, Color>,
) -> std::collections::HashMap<TShapeId, Color> {
	let mut colormap = std::collections::HashMap::new();
	let old_faces = FaceIterator::new(ffi::explore_faces(old_inner));
	let new_faces = FaceIterator::new(ffi::explore_faces(new_inner));
	for (old_face, new_face) in old_faces.zip(new_faces) {
		if let Some(&color) = old_colormap.get(&old_face.tshape_id()) {
			colormap.insert(new_face.tshape_id(), color);
		}
	}
	colormap
}

#[cfg(feature = "color")]
fn merge_colormaps(
	from_a: &[u64],
	from_b: &[u64],
	colormap_a: &std::collections::HashMap<TShapeId, Color>,
	colormap_b: &std::collections::HashMap<TShapeId, Color>,
) -> std::collections::HashMap<TShapeId, Color> {
	let mut result = std::collections::HashMap::new();
	for pair in from_a.chunks(2) {
		if let Some(&color) = colormap_a.get(&TShapeId(pair[1])) {
			result.insert(TShapeId(pair[0]), color);
		}
	}
	for pair in from_b.chunks(2) {
		if let Some(&color) = colormap_b.get(&TShapeId(pair[1])) {
			result.insert(TShapeId(pair[0]), color);
		}
	}
	result
}

// ==================== BooleanShape ====================

/// Result of a boolean operation.
pub struct Boolean {
	pub solids: Vec<Solid>,
	from_a: Vec<u64>,
	from_b: Vec<u64>,
}

impl Boolean {
	/// Returns `true` if `face` originated from the `other` (tool) operand.
	pub fn is_tool_face(&self, face: &crate::occt::face::Face) -> bool {
		self.from_b.contains(&face.tshape_id().0)
	}

	/// Returns `true` if `face` originated from `self` (the base shape operand).
	pub fn is_shape_face(&self, face: &crate::occt::face::Face) -> bool {
		self.from_a.contains(&face.tshape_id().0)
	}

	// --- Boolean operations ---

	pub fn union<'a>(
		a: impl IntoIterator<Item = &'a Solid> + Clone,
		b: impl IntoIterator<Item = &'a Solid> + Clone,
	) -> Result<Self, Error> {
		let c_self = to_compound(a.clone());
		let c_other = to_compound(b.clone());
		let r = ffi::boolean_fuse(&c_self, &c_other);
		if r.is_null() {
			return Err(Error::BooleanOperationFailed);
		}
		Self::build_boolean_result(r, a, b)
	}

	pub fn subtract<'a>(
		a: impl IntoIterator<Item = &'a Solid> + Clone,
		b: impl IntoIterator<Item = &'a Solid> + Clone,
	) -> Result<Self, Error> {
		let c_self = to_compound(a.clone());
		let c_other = to_compound(b.clone());
		let r = ffi::boolean_cut(&c_self, &c_other);
		if r.is_null() {
			return Err(Error::BooleanOperationFailed);
		}
		Self::build_boolean_result(r, a, b)
	}

	pub fn intersect<'a>(
		a: impl IntoIterator<Item = &'a Solid> + Clone,
		b: impl IntoIterator<Item = &'a Solid> + Clone,
	) -> Result<Self, Error> {
		let c_self = to_compound(a.clone());
		let c_other = to_compound(b.clone());
		let r = ffi::boolean_common(&c_self, &c_other);
		if r.is_null() {
			return Err(Error::BooleanOperationFailed);
		}
		Self::build_boolean_result(r, a, b)
	}

	// ==================== Boolean helper ====================

	fn build_boolean_result<'a>(
		r: cxx::UniquePtr<ffi::BooleanShape>,
		self_solids: impl IntoIterator<Item = &'a Solid>,
		other_solids: impl IntoIterator<Item = &'a Solid>,
	) -> Result<Boolean, Error> {
		let from_a = ffi::boolean_shape_from_a(&r);
		let from_b = ffi::boolean_shape_from_b(&r);
		let inner = ffi::boolean_shape_shape(&r);

		#[cfg(feature = "color")]
		let colormap = {
			let colormap_a = merge_all_colormaps(self_solids);
			let colormap_b = merge_all_colormaps(other_solids);
			merge_colormaps(&from_a, &from_b, &colormap_a, &colormap_b)
		};
		#[cfg(not(feature = "color"))]
		let _ = (self_solids, other_solids);

		let solids = decompose(
			&inner,
			#[cfg(feature = "color")]
			&colormap,
		);

		Ok(Boolean {
			solids,
			from_a,
			from_b,
		})
	}
}

impl From<Boolean> for Vec<Solid> {
	fn from(r: Boolean) -> Vec<Solid> {
		r.solids
	}
}
use crate::common::error::Error;
use super::ffi;
use super::compound::Compound;
use super::solid::Solid;
#[cfg(feature = "color")]
use crate::common::color::Color;

// ==================== Color helpers ====================

#[cfg(feature = "color")]
fn merge_colormaps(
	from_a: &[u64],
	from_b: &[u64],
	colormap_a: &std::collections::HashMap<u64, Color>,
	colormap_b: &std::collections::HashMap<u64, Color>,
) -> std::collections::HashMap<u64, Color> {
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
		self.from_b.contains(&face.tshape_id())
	}

	/// Returns `true` if `face` originated from `self` (the base shape operand).
	pub fn is_shape_face(&self, face: &crate::occt::face::Face) -> bool {
		self.from_a.contains(&face.tshape_id())
	}

	// --- Boolean operations ---

	pub fn union<'a>(
		a: impl IntoIterator<Item = &'a Solid>,
		b: impl IntoIterator<Item = &'a Solid>,
	) -> Result<Self, Error> {
		let ca = Compound::new(a);
		let cb = Compound::new(b);
		let r = ffi::boolean_fuse(ca.inner(), cb.inner());
		if r.is_null() {
			return Err(Error::BooleanOperationFailed);
		}
		Self::build_boolean_result(r, ca, cb)
	}

	pub fn subtract<'a>(
		a: impl IntoIterator<Item = &'a Solid>,
		b: impl IntoIterator<Item = &'a Solid>,
	) -> Result<Self, Error> {
		let ca = Compound::new(a);
		let cb = Compound::new(b);
		let r = ffi::boolean_cut(ca.inner(), cb.inner());
		if r.is_null() {
			return Err(Error::BooleanOperationFailed);
		}
		Self::build_boolean_result(r, ca, cb)
	}

	pub fn intersect<'a>(
		a: impl IntoIterator<Item = &'a Solid>,
		b: impl IntoIterator<Item = &'a Solid>,
	) -> Result<Self, Error> {
		let ca = Compound::new(a);
		let cb = Compound::new(b);
		let r = ffi::boolean_common(ca.inner(), cb.inner());
		if r.is_null() {
			return Err(Error::BooleanOperationFailed);
		}
		Self::build_boolean_result(r, ca, cb)
	}

	// ==================== Boolean helper ====================

	fn build_boolean_result(
		r: cxx::UniquePtr<ffi::BooleanShape>,
		ca: Compound,
		cb: Compound,
	) -> Result<Boolean, Error> {
		let from_a = ffi::boolean_shape_from_a(&r);
		let from_b = ffi::boolean_shape_from_b(&r);
		let inner = ffi::boolean_shape_shape(&r);

		#[cfg(feature = "color")]
		let colormap = merge_colormaps(&from_a, &from_b, ca.colormap(), cb.colormap());

		let compound = Compound::from_raw(
			inner,
			#[cfg(feature = "color")]
			colormap,
		);

		Ok(Boolean {
			solids: compound.decompose(),
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
use glam::DVec3;
use crate::traits::FaceTrait;

/// A face in the pure Rust backend.
///
/// Stores precomputed normal and center of mass.
#[derive(Debug, Clone)]
pub struct Face {
	pub(crate) normal: DVec3,
	pub(crate) center: DVec3,
}

impl FaceTrait for Face {
	fn normal_at_center(&self) -> DVec3 {
		self.normal
	}

	fn center_of_mass(&self) -> DVec3 {
		self.center
	}
}

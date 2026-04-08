use super::error::Error;
use crate::{Solid, SolidExt};
use glam::DVec3;

/// Extrude only the tool-side faces of a boolean result by `delta` to create a filler solid.
fn extrude_tool_faces(solids: &[Solid], metadata: &[Vec<u64>; 2], delta: DVec3) -> Result<Vec<Solid>, Error> {
	let mut filler: Vec<Solid> = Vec::new();
	for face in solids.iter().flat_map(|s| s.face_iter()).filter(|f| metadata[1].contains(&f.tshape_id())) {
		let solid = face.extrude(delta)?;
		filler = if filler.is_empty() {
			vec![solid]
		} else {
			filler.union(&[solid])?
		};
	}
	Ok(filler)
}

/// Cut `shape` with the plane through `origin` with normal `plane_normal`, then revolve
/// the cut face around `axis_direction` through `origin` by `angle` radians.
pub fn revolve_section(solids: &[Solid], origin: DVec3, axis_direction: DVec3, plane_normal: DVec3, angle: f64) -> Result<Vec<Solid>, Error> {
	let half = [Solid::half_space(origin, -plane_normal.normalize())];
	let (intersect_solids, intersect_meta) = solids.to_vec().intersect_with_metadata(&half)?;

	let mut result: Vec<Solid> = Vec::new();
	for face in intersect_solids.iter().flat_map(|s| s.face_iter()).filter(|f| intersect_meta[1].contains(&f.tshape_id())) {
		let solid = face.revolve(origin, axis_direction, angle)?;
		result = if result.is_empty() {
			vec![solid]
		} else {
			result.union(&[solid])?
		};
	}
	Ok(result)
}

/// Cut `shape` with the plane through `origin` with normal `plane_normal`, then sweep
/// the cut face along a helix around `axis_direction` through `origin`.
pub fn helix_section(shape: &[Solid], origin: DVec3, axis_direction: DVec3, plane_normal: DVec3, pitch: f64, turns: f64) -> Result<Vec<Solid>, Error> {
	let half = vec![Solid::half_space(origin, -plane_normal.normalize())];
	let (intersect_solids, intersect_meta) = shape.to_vec().intersect_with_metadata(&half)?;

	let mut result: Vec<Solid> = Vec::new();
	for face in intersect_solids.iter().flat_map(|s| s.face_iter()).filter(|f| intersect_meta[1].contains(&f.tshape_id())) {
		let solid = face.helix(origin, axis_direction, pitch, turns, false)?;
		result = if result.is_empty() {
			vec![solid]
		} else {
			result.union(&[solid])?
		};
	}
	Ok(result)
}

/// Split `shape` at `origin` along `delta`, translate one half by `delta`,
/// and fill the gap with an extruded filler derived from the cut face.
pub fn stretch_vector(solids: &[Solid], origin: DVec3, delta: DVec3) -> Result<Vec<Solid>, Error> {
	let half = [Solid::half_space(origin, -delta.normalize())];
	let solids = solids.to_vec();
	let (intersect_solids, intersect_meta) = solids.clone().intersect_with_metadata(&half)?;
	let part_pos = solids.subtract(&half)?.translate(delta);
	let filler = extrude_tool_faces(&intersect_solids, &intersect_meta, delta)?;
	intersect_solids.union(&filler)?.union(&part_pos)
}

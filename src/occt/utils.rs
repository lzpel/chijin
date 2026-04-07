use super::solid::Solid;
use crate::common::error::Error;
use crate::traits::SolidStruct;
use glam::DVec3;

/// Extrude only the tool-side faces of a boolean result by `delta` to create a filler solid.
fn extrude_tool_faces(solids: &[Solid], metadata: &[Vec<u64>; 2], delta: DVec3) -> Result<Vec<Solid>, Error> {
	let mut filler: Option<Vec<Solid>> = None;
	for face in solids.iter().flat_map(|s| s.face_iter()).filter(|f| metadata[1].contains(&f.tshape_id())) {
		let solid = face.extrude(delta)?;
		let extruded: Vec<Solid> = vec![solid];
		filler = Some(match filler {
			None => extruded,
			Some(f) => Solid::boolean_union(&f, &extruded)?.0,
		});
	}
	Ok(filler.unwrap_or_default())
}

/// Cut `shape` with the plane through `origin` with normal `plane_normal`, then revolve
/// the cut face around `axis_direction` through `origin` by `angle` radians.
pub fn revolve_section(shape: &[Solid], origin: DVec3, axis_direction: DVec3, plane_normal: DVec3, angle: f64) -> Result<Vec<Solid>, Error> {
	let half = vec![Solid::half_space(origin, -plane_normal.normalize())];
	let (intersect_solids, intersect_meta) = Solid::boolean_intersect(shape, &half)?;

	let mut result: Option<Vec<Solid>> = None;
	for face in intersect_solids.iter().flat_map(|s| s.face_iter()).filter(|f| intersect_meta[1].contains(&f.tshape_id())) {
		let solid = face.revolve(origin, axis_direction, angle)?;
		let revolved = vec![solid];
		result = Some(match result {
			None => revolved,
			Some(r) => Solid::boolean_union(&r, &revolved)?.0,
		});
	}
	Ok(result.unwrap_or_default())
}

/// Cut `shape` with the plane through `origin` with normal `plane_normal`, then sweep
/// the cut face along a helix around `axis_direction` through `origin`.
pub fn helix_section(shape: &[Solid], origin: DVec3, axis_direction: DVec3, plane_normal: DVec3, pitch: f64, turns: f64) -> Result<Vec<Solid>, Error> {
	let half = vec![Solid::half_space(origin, -plane_normal.normalize())];
	let (intersect_solids, intersect_meta) = Solid::boolean_intersect(shape, &half)?;

	let mut result: Option<Vec<Solid>> = None;
	for face in intersect_solids.iter().flat_map(|s| s.face_iter()).filter(|f| intersect_meta[1].contains(&f.tshape_id())) {
		let solid = face.helix(origin, axis_direction, pitch, turns, false)?;
		let swept = vec![solid];
		result = Some(match result {
			None => swept,
			Some(r) => Solid::boolean_union(&r, &swept)?.0,
		});
	}
	Ok(result.unwrap_or_default())
}

/// Split `shape` at `origin` along `delta`, translate one half by `delta`,
/// and fill the gap with an extruded filler derived from the cut face.
pub fn stretch_vector(shape: &[Solid], origin: DVec3, delta: DVec3) -> Result<Vec<Solid>, Error> {
	let half = vec![Solid::half_space(origin, -delta.normalize())];

	let (intersect_solids, intersect_meta) = Solid::boolean_intersect(shape, &half)?;
	let part_pos: Vec<Solid> = Solid::boolean_subtract(shape, &half)?.0.into_iter().map(|s| s.translate(delta)).collect();

	let filler = extrude_tool_faces(&intersect_solids, &intersect_meta, delta)?;
	let combined: Vec<Solid> = Solid::boolean_union(&intersect_solids, &filler)?.0;
	Ok(Solid::boolean_union(&combined, &part_pos)?.0)
}

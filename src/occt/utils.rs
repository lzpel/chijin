use crate::common::error::Error;
use crate::traits::SolidTrait;
use super::shape::{Boolean, to_compound};
use super::solid::Solid;
use super::ffi;
use super::iterators::FaceIterator;
use glam::DVec3;

/// Extrude only the tool-side faces of a boolean result by `delta` to create a filler solid.
fn extrude_tool_faces(result: &Boolean, delta: DVec3) -> Result<Vec<Solid>, Error> {
	let compound = to_compound(&result.solids);
	let mut filler: Option<Vec<Solid>> = None;
	for face in FaceIterator::new(ffi::explore_faces(&compound)).filter(|f| result.is_tool_face(f)) {
		let solid = face.extrude(delta)?;
		let extruded: Vec<Solid> = vec![solid];
		filler = Some(match filler {
			None => extruded,
			Some(f) => Boolean::union(&f, &extruded)?.into(),
		});
	}
	Ok(filler.unwrap_or_default())
}

/// Cut `shape` with the plane through `origin` with normal `plane_normal`, then revolve
/// the cut face around `axis_direction` through `origin` by `angle` radians.
pub fn revolve_section(
	shape: &[Solid],
	origin: DVec3,
	axis_direction: DVec3,
	plane_normal: DVec3,
	angle: f64,
) -> Result<Vec<Solid>, Error> {
	let half = vec![Solid::half_space(origin, -plane_normal.normalize())];
	let intersect_result = Boolean::intersect(shape, &half)?;

	let compound = to_compound(&intersect_result.solids);
	let mut result: Option<Vec<Solid>> = None;
	for face in FaceIterator::new(ffi::explore_faces(&compound))
		.filter(|f| intersect_result.is_tool_face(f))
	{
		let solid = face.revolve(origin, axis_direction, angle)?;
		let revolved = vec![solid];
		result = Some(match result {
			None => revolved,
			Some(r) => Boolean::union(&r, &revolved)?.into(),
		});
	}
	Ok(result.unwrap_or_default())
}

/// Cut `shape` with the plane through `origin` with normal `plane_normal`, then sweep
/// the cut face along a helix around `axis_direction` through `origin`.
pub fn helix_section(
	shape: &[Solid],
	origin: DVec3,
	axis_direction: DVec3,
	plane_normal: DVec3,
	pitch: f64,
	turns: f64,
) -> Result<Vec<Solid>, Error> {
	let half = vec![Solid::half_space(origin, -plane_normal.normalize())];
	let intersect_result = Boolean::intersect(shape, &half)?;

	let compound = to_compound(&intersect_result.solids);
	let mut result: Option<Vec<Solid>> = None;
	for face in FaceIterator::new(ffi::explore_faces(&compound))
		.filter(|f| intersect_result.is_tool_face(f))
	{
		let solid = face.helix(origin, axis_direction, pitch, turns, false)?;
		let swept = vec![solid];
		result = Some(match result {
			None => swept,
			Some(r) => Boolean::union(&r, &swept)?.into(),
		});
	}
	Ok(result.unwrap_or_default())
}

/// Split `shape` at `origin` along `delta`, translate one half by `delta`,
/// and fill the gap with an extruded filler derived from the cut face.
pub fn stretch_vector(shape: &[Solid], origin: DVec3, delta: DVec3) -> Result<Vec<Solid>, Error> {
	let half = vec![Solid::half_space(origin, -delta.normalize())];

	let intersect_result = Boolean::intersect(shape, &half)?;
	let part_pos: Vec<Solid> = Boolean::subtract(shape, &half)?
		.solids
		.into_iter()
		.map(|s| s.translate(delta))
		.collect();

	let filler = extrude_tool_faces(&intersect_result, delta)?;
	let part_neg: Vec<Solid> = intersect_result.into();
	let combined: Vec<Solid> = Boolean::union(&part_neg, &filler)?.into();
	Boolean::union(&combined, &part_pos).map(Vec::from)
}

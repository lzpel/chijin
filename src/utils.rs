use crate::error::Error;
use crate::shape::{BooleanShape, Shape};
use crate::solid::Solid;
use glam::DVec3;

/// ツール側フェイスだけを delta 方向に押し出してフィラーを作ります。
fn extrude_tool_faces(result: &BooleanShape, delta: DVec3) -> Result<Vec<Solid>, Error> {
	let mut filler: Option<Vec<Solid>> = None;
	for face in result.solids.faces().filter(|f| result.is_tool_face(f)) {
		let solid = face.extrude(delta)?;
		let extruded: Vec<Solid> = vec![solid];
		filler = Some(match filler {
			None => extruded,
			Some(f) => f.union(&extruded)?.into(),
		});
	}
	Ok(filler.unwrap_or_default())
}

/// `origin` を含み `plane_normal` が法線の平面で `shape` を切断し、
/// その断面を `origin` を通る `axis_dir` 軸周りに `angle` だけ回転させた回転体を返す。
pub fn revolve_section(
	shape: &[Solid],
	origin: DVec3,
	axis_direction: DVec3,
	plane_normal: DVec3,
	angle: f64,
) -> Result<Vec<Solid>, Error> {
	let half = vec![Solid::half_space(origin, -plane_normal.normalize())];
	let intersect_result = shape.intersect(&half)?;

	let mut result: Option<Vec<Solid>> = None;
	for face in intersect_result.solids.faces().filter(|f| intersect_result.is_tool_face(f)) {
		let solid = face.revolve(origin, axis_direction, angle)?;
		let revolved = vec![solid];
		result = Some(match result {
			None => revolved,
			Some(r) => r.union(&revolved)?.into(),
		});
	}
	Ok(result.unwrap_or_default())
}

/// `origin` を含み `plane_normal` が法線の平面で `shape` を切断し、
/// その断面を `origin` を通る `axis_dir` 軸周りにヘリカルスイープした形状を返す。
pub fn helix_section(
	shape: &[Solid],
	origin: DVec3,
	axis_direction: DVec3,
	plane_normal: DVec3,
	pitch: f64,
	turns: f64,
) -> Result<Vec<Solid>, Error> {
	let half = vec![Solid::half_space(origin, -plane_normal.normalize())];
	let intersect_result = shape.intersect(&half)?;

	let mut result: Option<Vec<Solid>> = None;
	for face in intersect_result.solids.faces().filter(|f| intersect_result.is_tool_face(f)) {
		let solid = face.helix(origin, axis_direction, pitch, turns, false)?;
		let swept = vec![solid];
		result = Some(match result {
			None => swept,
			Some(r) => r.union(&swept)?.into(),
		});
	}
	Ok(result.unwrap_or_default())
}

/// 指定された座標とベクトルで形状を分割し、片方を平行移動させた後、隙間を押し出し形状で埋めることで引き伸ばしを行います。
pub fn stretch_vector(shape: &[Solid], origin: DVec3, delta: DVec3) -> Result<Vec<Solid>, Error> {
	let half = vec![Solid::half_space(origin, -delta.normalize())];

	let intersect_result = shape.intersect(&half)?;
	let part_pos: Vec<Solid> = shape.subtract(&half)?.into();
	let part_pos = part_pos.translated(delta);

	let filler = extrude_tool_faces(&intersect_result, delta)?;
	let part_neg: Vec<Solid> = intersect_result.into();
	let combined: Vec<Solid> = part_neg.union(&filler)?.into();
	combined.union(&part_pos).map(Vec::from)
}

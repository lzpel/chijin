//! Build a chijin (hand drum from Amami Oshima) with colors, boolean ops, and SVG export.

use cadrum::{Color, Compound, DVec3, Edge, ProfileOrient, Solid};
use std::f64::consts::PI;

pub fn chijin() -> Result<Solid, cadrum::Error> {
	// ── Body (cylinder): r=15, h=8, centered at origin (z=-4..+4) ────────
	let cylinder = Solid::cylinder(15.0, DVec3::Z, 8.0)
		.translate(DVec3::new(0.0, 0.0, -4.0))
		.color("#999");

	// ── Sheet: closed polygon in the XZ plane (y=0), swept 360° around Z
	// により面と縁を一体で生成する。Face::from_polygon + Face::revolve の置換版:
	//   - Edge::polygon は最後の点 → 最初の点を自動補完して閉じる
	//   - spine は Z 軸まわりの円。半径によらずプロファイルを Z 周りに純粋回転
	//     させるだけなので任意の正の値で可
	//   - ProfileOrient::Up(Z) でプロファイルの上方向を Z 固定 → 回転(revolve)と等価
	let cross_section = Edge::polygon(&[
		DVec3::new(0.0, 0.0, 5.0),
		DVec3::new(15.0, 0.0, 5.0),
		DVec3::new(17.0, 0.0, 3.0),
		DVec3::new(15.0, 0.0, 4.0),
		DVec3::new(0.0, 0.0, 4.0),
	])?;
	let spine = Edge::circle(1.0, DVec3::Z)?;
	let sheet = Solid::sweep(&cross_section, &[spine], ProfileOrient::Up(DVec3::Z))?
		.color("#fff");
	let sheets = [sheet.clone().mirror(DVec3::ZERO, DVec3::Z), sheet];

	// ── Lacing blocks: 2x8x1, rotated 60° around Z, placed at y=15 ──────
	let block_proto = Solid::cube(2.0, 8.0, 1.0)
		.translate(DVec3::new(-1.0, -4.0, -0.5))
		.rotate_z(60.0_f64.to_radians())
		.translate(DVec3::new(0.0, 15.0, 0.0));

	// ── Lacing holes: thin cylinders through each block ──────────────────
	let hole_proto = Solid::cylinder(0.7, DVec3::new(10.0, 0.0, 30.0), 30.0)
		.translate(DVec3::new(-5.0, 16.0, -15.0));

	// Distribute N blocks and holes evenly around Z, each block in a rainbow color
	// N 個のブロックと穴を Z 軸周りに等間隔配置、各ブロックに虹色を割り当て
	const N: usize = 20;
	let angle = |i: usize| 2.0 * PI * (i as f64) / (N as f64);
	let color = |i: usize| Color::from_hsv(i as f32 / N as f32, 1.0, 1.0);
	let blocks: [Solid; N] = std::array::from_fn(|i| block_proto.clone().rotate_z(angle(i)).color(color(i)));
	let holes: [Solid; N] = std::array::from_fn(|i| hole_proto.clone().rotate_z(angle(i)));
	// ── Assemble with boolean operations: union, subtract, union ─────────
	let result = [cylinder]
		.union(&sheets)?
		.subtract(&holes)?
		.union(&blocks)?;
	assert!(result.len() == 1);
	Ok(result.into_iter().next().unwrap())
}

fn main() -> Result<(), cadrum::Error> {
	let example_name = std::path::Path::new(file!()).file_stem().unwrap().to_str().unwrap();
	let result = [chijin()?];

	let step_path = format!("{example_name}.step");
	let mut f = std::fs::File::create(&step_path).expect("failed to create STEP file");
	cadrum::write_step(&result, &mut f).expect("failed to write STEP");
	println!("wrote {step_path}");

	let svg_path = format!("{example_name}.svg");
	let mut f = std::fs::File::create(&svg_path).expect("failed to create SVG file");
	cadrum::mesh(&result, 0.5).and_then(|m| m.write_svg(DVec3::new(1.0, 1.0, 1.0), true, false, &mut f)).expect("failed to write SVG");
	println!("wrote {svg_path}");

	Ok(())
}

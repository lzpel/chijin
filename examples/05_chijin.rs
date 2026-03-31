//! Chijin example: build a chijin (hand drum from Amami Oshima) using the chijin library.
//!
//! ```
//! cargo run --example chijin
//! ```
//!
//! Output: chijin.step (AP214 STEP, colored), chijin.svg

use cadrum::{Boolean, Face, Color, Shape, Solid};
use glam::DVec3;
use std::f64::consts::PI;

pub fn chijin() -> Solid {
	// ── Body (cylinder): r=15, h=8, centered at origin (z=-4..+4) ────────
	let cylinder: Solid =
		Solid::cylinder(DVec3::new(0.0, 0.0, -4.0), 15.0, DVec3::Z, 8.0).color_paint(Some(Color::from_hex("#999").unwrap()));

	// ── Rim: cross-section polygon in the x=0 plane, revolved 360° around Z
	// to form a ring with outer radius 17 at z=3..5.
	// Mirrored across z=0 to create rims on both top and bottom.
	let cross_section = Face::from_polygon(&[
		DVec3::new(0.0, 0.0, 5.0),
		DVec3::new(0.0, 15.0, 5.0),
		DVec3::new(0.0, 17.0, 3.0),
		DVec3::new(0.0, 15.0, 4.0),
		DVec3::new(0.0, 0.0, 4.0),
		DVec3::new(0.0, 0.0, 5.0),
	])
	.unwrap();
	let sheet = cross_section
		.revolve(DVec3::ZERO, DVec3::Z, 2.0 * PI)
		.unwrap()
		.color_paint(Some(Color::from_hex("#fff").unwrap()));
	let sheets = [sheet.mirrored(DVec3::ZERO, DVec3::Z), sheet];

	// ── Lacing blocks: 2x8x1, rotated 60° around Z, placed at y=15 ──────
	let block_proto = Solid::box_from_corners(DVec3::new(-1.0, -4.0, -0.5), DVec3::new(1.0, 4.0, 0.5))
		.rotate(DVec3::ZERO, DVec3::Z, 60.0_f64.to_radians())
		.translate(DVec3::new(0.0, 15.0, 0.0));

	// ── Lacing holes: thin cylinders through each block ──────────────────
	let hole_proto = Solid::cylinder(
		DVec3::new(-5.0, 16.0, -15.0),
		0.7,
		DVec3::new(10.0, 0.0, 30.0),
		30.0,
	);

	// Distribute 20 blocks and holes evenly around Z, each block in a rainbow color
	let n = 20usize;
	let mut blocks: Vec<Solid> = Vec::with_capacity(n);
	let mut holes: Vec<Solid> = Vec::with_capacity(n);
	for i in 0..n {
		let angle = 2.0 * PI * (i as f64) / (n as f64);
		let color = Color::from_hsv(i as f32 / n as f32, 1.0, 1.0);
		blocks.push(
			block_proto
				.clone()
				.rotate(DVec3::ZERO, DVec3::Z, angle)
				.color_paint(Some(color)),
		);
		holes.push(hole_proto.clone().rotate(DVec3::ZERO, DVec3::Z, angle));
	}
	let blocks = blocks
		.into_iter()
		.map(|v| vec![v])
		.reduce(|a, b| Boolean::union(&a, &b).unwrap().solids)
		.unwrap();
	let holes = holes
		.into_iter()
		.map(|v| vec![v])
		.reduce(|a, b| Boolean::union(&a, &b).unwrap().solids)
		.unwrap();

	// ── Assemble with boolean operations: union, subtract, union ─────────
	let combined: Vec<Solid> = Boolean::union(&[cylinder], &sheets)
		.expect("cylinder + sheet union failed")
		.into();
	let result: Vec<Solid> = Boolean::subtract(&combined, &holes).unwrap().into(); // drill holes
	let result: Vec<Solid> = Boolean::union(&result, &blocks).unwrap().into(); // attach blocks
	assert!(result.len() == 1);
	result.into_iter().next().unwrap()
}

fn main() {
	let example_name = std::path::Path::new(file!()).file_stem().unwrap().to_str().unwrap();
	let result = vec![chijin()];
	// ── Write STEP ───────────────────────────────────────────────────────
	let step_path = format!("{example_name}.step");
	let mut f = std::fs::File::create(&step_path).expect("failed to create STEP file");
	cadrum::write_step_with_colors(&result, &mut f).expect("failed to write STEP");
	println!("wrote {}", &step_path);

	// ── Write SVG (isometric view from (1,1,1)) ─────────────────────────
	let svg_path = format!("{}.svg", example_name);
	let svg = result
		.to_svg(DVec3::new(1.0, 1.0, 1.0), 0.5)
		.expect("failed to export SVG");
	let mut f = std::fs::File::create(&svg_path).expect("failed to create SVG file");
	std::io::Write::write_all(&mut f, svg.as_bytes()).expect("failed to write SVG");
	println!("wrote {}", &svg_path);
}

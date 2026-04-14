//! Demo of `Solid::extrude`: push a closed 2D profile along a direction vector.
//!
//! - **Box**: square polygon extruded along Z
//! - **Oblique cylinder**: circle extruded at a steep angle
//! - **L-beam**: L-shaped polygon extruded along Z
//! - **Heart**: BSpline heart-shaped profile extruded along Z

use cadrum::{BSplineEnd, Edge, Error, Solid};
use glam::DVec3;

/// Square polygon → box (simplest extrude).
fn build_box() -> Result<Solid, Error> {
	let profile = Edge::polygon(&[
		DVec3::new(0.0, 0.0, 0.0),
		DVec3::new(5.0, 0.0, 0.0),
		DVec3::new(5.0, 5.0, 0.0),
		DVec3::new(0.0, 5.0, 0.0),
	])?;
	Solid::extrude(&profile, DVec3::new(0.0, 0.0, 8.0))
}

/// Circle extruded at a steep angle → oblique cylinder.
fn build_oblique_cylinder() -> Result<Solid, Error> {
	let profile = [Edge::circle(3.0, DVec3::Z)?];
	Solid::extrude(&profile, DVec3::new(-4.0, 6.0, 8.0))
}

/// L-shaped polygon → L-beam.
fn build_l_beam() -> Result<Solid, Error> {
	let profile = Edge::polygon(&[
		DVec3::new(0.0, 0.0, 0.0),
		DVec3::new(4.0, 0.0, 0.0),
		DVec3::new(4.0, 1.0, 0.0),
		DVec3::new(1.0, 1.0, 0.0),
		DVec3::new(1.0, 3.0, 0.0),
		DVec3::new(0.0, 3.0, 0.0),
	])?;
	Solid::extrude(&profile, DVec3::new(0.0, 0.0, 12.0))
}

/// Heart-shaped BSpline profile extruded along Z.
fn build_heart() -> Result<Solid, Error> {
	let profile = [Edge::bspline(
		&[
			DVec3::new(0.0, -4.0, 0.0),   // bottom tip
			DVec3::new(2.0, -1.5, 0.0),
			DVec3::new(4.0, 1.5, 0.0),
			DVec3::new(2.5, 3.5, 0.0),    // right lobe top
			DVec3::new(0.0, 2.0, 0.0),    // center dip
			DVec3::new(-2.5, 3.5, 0.0),   // left lobe top
			DVec3::new(-4.0, 1.5, 0.0),
			DVec3::new(-2.0, -1.5, 0.0),
		],
		BSplineEnd::Periodic,
	)?];
	Solid::extrude(&profile, DVec3::new(0.0, 0.0, 7.0))
}

fn main() -> Result<(), Error> {
	let example_name = std::path::Path::new(file!()).file_stem().unwrap().to_str().unwrap();

	let box_solid = build_box()?.color("#b0d4f1");
	let oblique = build_oblique_cylinder()?.color("#f1c8b0").translate(DVec3::new(12.0, 0.0, 0.0));
	let l_beam = build_l_beam()?.color("#b0f1c8").translate(DVec3::new(28.0, 0.0, 0.0));
	let heart = build_heart()?.color("#f1b0b0").translate(DVec3::new(38.0, 0.0, 0.0));

	let result = [box_solid, oblique, l_beam, heart];

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

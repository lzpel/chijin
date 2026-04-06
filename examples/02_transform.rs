use cadrum::{Color, Solid};
use glam::DVec3;
use std::f64::consts::PI;

fn main() {
	let example_name = std::path::Path::new(file!())
		.file_stem()
		.unwrap()
		.to_str()
		.unwrap();

	// Base shape: cone pointing up (tip at Z=20), used as reference for each transform
	let base = || {
		Solid::cone(DVec3::ZERO, DVec3::Z, 8.0, 0.0, 20.0)
			.color_paint(Some(Color::from_str("#888888").unwrap()))
	};

	// original — reference, no transform
	let original = base().translate(DVec3::new(0.0, 0.0, 0.0));

	// translate — shift +20 along Z
	let translated = base()
		.color_paint(Some(Color::from_str("#4a90d9").unwrap()))
		.translate(DVec3::new(40.0, 0.0, 20.0));

	// rotate — 90° around X axis so the cone tips toward Y
	let rotated = base()
		.color_paint(Some(Color::from_str("#e67e22").unwrap()))
		.rotate(DVec3::new(80.0, 0.0, 0.0), DVec3::X, PI / 2.0)
		.translate(DVec3::new(80.0, 0.0, 0.0));

	// scaled — 1.5x from its local origin
	let scaled = base()
		.color_paint(Some(Color::from_str("#2ecc71").unwrap()))
		.scaled(DVec3::ZERO, 1.5)
		.translate(DVec3::new(120.0, 0.0, 0.0));

	// mirrored — flip across Z=0 plane so the tip points down
	let mirrored = base()
		.color_paint(Some(Color::from_str("#e74c3c").unwrap()))
		.mirrored(DVec3::ZERO, DVec3::Z)
		.translate(DVec3::new(160.0, 0.0, 0.0));

	let shapes = vec![original, translated, rotated, scaled, mirrored];

	let mut f =
		std::fs::File::create(format!("{example_name}.step")).expect("failed to create file");
	cadrum::io::write_step(&shapes, &mut f).expect("failed to write STEP");

	let mut svg =
		std::fs::File::create(format!("{example_name}.svg")).expect("failed to create SVG file");
	cadrum::io::write_svg(&shapes, DVec3::new(1.0, 1.0, 1.0), 0.5, &mut svg)
		.expect("failed to write SVG");
}

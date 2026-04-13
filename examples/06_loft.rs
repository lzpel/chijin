//! Demo of `Solid::loft`: skin a smooth solid through cross-section wires.
//!
//! - **Frustum**: two circles of different radii → truncated cone (minimal loft)
//! - **Morph**: square polygon → circle (cross-section shape transition)
//! - **Tilted**: three non-parallel circular sections → twisted loft

use cadrum::{Edge, Error, Solid};
use glam::DVec3;

/// Two circles → frustum (minimal loft example).
fn build_frustum() -> Result<Solid, Error> {
	let lower = [Edge::circle(3.0, DVec3::Z)?];
	let upper = [Edge::circle(1.5, DVec3::Z)?.translate(DVec3::Z * 8.0)];
	Ok(Solid::loft(&[lower, upper])?.color("#cd853f"))
}

/// Square polygon → circle (2-section morph loft).
fn build_morph() -> Result<Solid, Error> {
	let r = 2.5;
	let square = Edge::polygon(&[
		DVec3::new(-r, -r, 0.0),
		DVec3::new(r, -r, 0.0),
		DVec3::new(r, r, 0.0),
		DVec3::new(-r, r, 0.0),
	])?;
	let circle = Edge::circle(r, DVec3::Z)?.translate(DVec3::Z * 10.0);

	Ok(Solid::loft([square.as_slice(), std::slice::from_ref(&circle)])?.color("#808000"))
}

/// Three non-parallel circular sections → twisted loft.
fn build_tilted() -> Result<Solid, Error> {
	let bottom = [Edge::circle(2.5, DVec3::Z)?];
	let mid = [Edge::circle(2.0, DVec3::new(0.3, 0.0, 1.0).normalize())?
		.translate(DVec3::new(1.0, 0.0, 5.0))];
	let top = [Edge::circle(1.5, DVec3::new(-0.2, 0.3, 1.0).normalize())?
		.translate(DVec3::new(-0.5, 1.0, 10.0))];

	Ok(Solid::loft(&[bottom, mid, top])?.color("#4682b4"))
}

fn main() -> Result<(), Error> {
	let example_name = std::path::Path::new(file!()).file_stem().unwrap().to_str().unwrap();

	let frustum = build_frustum()?;
	let morph = build_morph()?.translate(DVec3::new(10.0, 0.0, 0.0));
	let tilted = build_tilted()?.translate(DVec3::new(20.0, 0.0, 0.0));

	let result = [frustum, morph, tilted];

	let step_path = format!("{example_name}.step");
	let mut f = std::fs::File::create(&step_path).expect("failed to create STEP file");
	cadrum::io::write_step(&result, &mut f).expect("failed to write STEP");
	println!("wrote {step_path}");

	let svg_path = format!("{example_name}.svg");
	let mut f = std::fs::File::create(&svg_path).expect("failed to create SVG file");
	cadrum::io::write_svg(&result, DVec3::new(1.0, 1.0, 1.0), 0.5, true, false, &mut f).expect("failed to write SVG");
	println!("wrote {svg_path}");

	Ok(())
}

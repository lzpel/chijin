//! Demo of `Solid::shell`:
//! - Cube: remove top face, offset inward → open-top container
//! - Torus: bisect with a half-space to introduce planar cut faces, then
//!   shell using those cut faces as the openings → thin-walled half-ring
//!   with both cross-sections exposed

use cadrum::{DVec3, Error, Solid};

fn hollow_cube() -> Result<Solid, Error> {
	let cube = Solid::cube(8.0, 8.0, 8.0);
	// TopExp_Explorer order on a box is stable; +Z face ends up last.
	let top = cube.iter_face().last().expect("cube has faces");
	cube.shell(-1.0, [top])
}

fn halved_shelled_torus(thickness: f64) -> Result<Solid, Error> {
	let torus = Solid::torus(6.0, 2.0, DVec3::Y);
	// Bisect with Y=0 half-space (normal +Y): keep the +Y half of the ring — always 1 solid.
	let cutter = Solid::half_space(DVec3::ZERO, -DVec3::Z);
	// from_cutter is a flat [post_id, src_id, ...]: post_ids are TShape addresses
	// in the result tree, src_ids live in the cutter tree. Both are globally
	// unique pointers, so `contains` works without separating even/odd indices.
	let (mut halves, [_, from_cutter]) = torus.intersect_with_metadata(&[cutter])?;
	let half = halves.pop().ok_or(Error::BooleanOperationFailed)?;
	half.shell(thickness, half.iter_face().filter(|f| from_cutter.contains(&f.tshape_id())))
}

fn main() -> Result<(), Error> {
	let example_name = std::path::Path::new(file!()).file_stem().unwrap().to_str().unwrap();

	let result = [
		hollow_cube()?.color("#d0a878"),
		halved_shelled_torus(1.0)?.color("#ff5e00").translate(DVec3::X * 18.0),
		halved_shelled_torus(-1.0)?.color("#0052ff").translate(DVec3::X * 18.0 + DVec3::Y * 10.0),
	];

	let mut f = std::fs::File::create(format!("{example_name}.step")).expect("failed to create STEP file");
	cadrum::write_step(&result, &mut f).expect("failed to write STEP");

	// Isometric view from (1, 1, 2) with shading so the cavity depth reads
	// naturally.
	let mut f = std::fs::File::create(format!("{example_name}.svg")).expect("failed to create SVG file");
	cadrum::mesh(&result, 0.2).and_then(|m| m.write_svg(DVec3::new(1.0, 1.0, 2.0), false, true, &mut f)).expect("failed to write SVG");

	println!("wrote {example_name}.step / {example_name}.svg");
	Ok(())
}

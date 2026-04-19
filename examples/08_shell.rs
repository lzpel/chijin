//! Demo of `Solid::shell`:
//! - Cube: remove top face, offset inward → open-top container
//! - Torus: bisect with a half-space to introduce planar cut faces, then
//!   shell using those cut faces as the openings → thin-walled half-ring
//!   with both cross-sections exposed

use cadrum::{DVec3, Error, Face, Solid};
use std::collections::HashSet;

fn hollow_cube() -> Result<Solid, Error> {
	let cube = Solid::cube(8.0, 8.0, 8.0);
	// TopExp_Explorer order on a box is stable; +Z face ends up last.
	let top = cube.iter_face().last().expect("cube has faces");
	cube.shell(-1.0, [top])
}

fn halved_shelled_torus() -> Result<Vec<Solid>, Error> {
	let torus = Solid::torus(6.0, 2.0, DVec3::Z);
	// Bisect with Y=0 half-space (normal +Y): keep the +Y half of the ring.
	let cutter = Solid::half_space(DVec3::ZERO, DVec3::Y);
	// Metadata variant returns [from_torus, from_cutter]: faces in the result
	// that originated from the cutter are exactly the planar cut disks.
	let (halves, [_, from_cutter]) = torus.intersect_with_metadata(&[cutter])?;
	let cut_ids: HashSet<u64> = from_cutter.chunks(2).map(|p| p[0]).collect();
	halves.into_iter().map(|half| {
		let cuts: Vec<&Face> = half.iter_face().filter(|f| cut_ids.contains(&f.tshape_id())).collect();
		half.shell(-0.3, cuts)
	}).collect()
}

fn main() -> Result<(), Error> {
	let example_name = std::path::Path::new(file!()).file_stem().unwrap().to_str().unwrap();

	let cube = hollow_cube()?.color("#d0a878");
	let torus_half: Vec<Solid> = halved_shelled_torus()?
		.into_iter()
		.map(|s| s.color("#a8c8d0").translate(DVec3::X * 18.0))
		.collect();
	let result: Vec<Solid> = std::iter::once(cube).chain(torus_half).collect();

	let mut f = std::fs::File::create(format!("{example_name}.step")).expect("failed to create STEP file");
	cadrum::write_step(&result, &mut f).expect("failed to write STEP");

	// Isometric view from (1, 1, 2) with shading so the cavity depth reads
	// naturally.
	let mut f = std::fs::File::create(format!("{example_name}.svg")).expect("failed to create SVG file");
	cadrum::mesh(&result, 0.2).and_then(|m| m.write_svg(DVec3::new(1.0, 1.0, 2.0), false, true, &mut f)).expect("failed to write SVG");

	println!("wrote {example_name}.step / {example_name}.svg");
	Ok(())
}

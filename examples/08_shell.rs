//! Demo of `Solid::shell` and related cutaway techniques:
//! - Cube: remove top face, offset inward → open-top container (real shell)
//! - Torus: OCCT's `MakeThickSolidByJoin` needs at least one open face to
//!   hollow a closed solid, so the ring is built by subtracting an inner torus
//!   from an outer torus, then bisected with a half-space to expose the
//!   cross-section.

use cadrum::{Compound, DVec3, Error, Solid};

fn hollow_cube() -> Result<Solid, Error> {
	let cube = Solid::cube(8.0, 8.0, 8.0);
	// TopExp_Explorer order on a box is stable; +Z face ends up last.
	let top = cube.iter_face().last().expect("cube has faces");
	cube.shell(-1.0, [top])
}

fn torus_ring_halved() -> Result<Vec<Solid>, Error> {
	// Hollow ring: outer torus minus inner torus → 0.3-thick wall.
	let outer = Solid::torus(6.0, 2.3, DVec3::Z);
	let inner = Solid::torus(6.0, 2.0, DVec3::Z);
	let ring = outer.subtract(&[inner])?;
	// Bisect with the Y=0 plane (half-space normal +Y) to expose the cross-section.
	let cutter = Solid::half_space(DVec3::ZERO, DVec3::Y);
	ring.intersect(&[cutter])
}

fn main() -> Result<(), Error> {
	let example_name = std::path::Path::new(file!()).file_stem().unwrap().to_str().unwrap();

	let cube = hollow_cube()?.color("#d0a878");
	let torus_half: Vec<Solid> = torus_ring_halved()?
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

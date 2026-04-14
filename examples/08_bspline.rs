use cadrum::Solid;
use glam::{DQuat, DVec3};
use std::f64::consts::TAU;

// 2 field-period stellarator-like torus.
// `Solid::bspline` is fed a 2D control-point grid to build a periodic B-spline solid.
// Every variation below is invariant under phi → phi+π (or shifts by a multiple
// of 2π), so the resulting shape has 180° rotational symmetry around the Z axis:
//   a(phi)       = 1.8 + 0.6 * sin(2φ)      radial semi-axis
//   b(phi)       = 1.0 + 0.4 * cos(2φ)      Z semi-axis
//   psi(phi)     = 2 * phi                  cross-section twist (2 turns per loop)
//   z_shift(phi) = 1.0 * sin(2φ)            vertical undulation
const M: usize = 48; // toroidal (U) — must be even for 180° symmetry
const N: usize = 24; // poloidal (V) — arbitrary
const RING_R: f64 = 6.0;

fn point(i: usize, j: usize) -> DVec3 {
	let phi = TAU * (i as f64) / (M as f64);
	let theta = TAU * (j as f64) / (N as f64);
	let two_phi = 2.0 * phi;
	let a = 1.8 + 0.6 * two_phi.sin();
	let b = 1.0 + 0.4 * two_phi.cos();
	let psi = two_phi; // twist: 2 full turns per toroidal loop
	let z_shift = 1.0 * two_phi.sin();
	// 1. Local cross-section (pre-twist ellipse in the (X, Z) plane)
	let local_raw = DVec3::X * (a * theta.cos()) + DVec3::Z * (b * theta.sin());
	// 2. Rotate by psi around the local Y axis (major-circle tangent) — the twist
	let local_twisted = DQuat::from_axis_angle(DVec3::Y, psi) * local_raw;
	// 3. Undulate vertically in the local frame
	let local_shifted = local_twisted + DVec3::Z * z_shift;
	// 4. Push outward along the major radius by RING_R
	let translated = local_shifted + DVec3::X * RING_R;
	// 5. Rotate the whole point around the global Z axis by phi
	DQuat::from_axis_angle(DVec3::Z, phi) * translated
}

fn main() {
	let example_name = std::path::Path::new(file!()).file_stem().unwrap().to_str().unwrap();

	let grid: [[DVec3; N]; M] = std::array::from_fn(|i| std::array::from_fn(|j| point(i, j)));
	let plasma = Solid::bspline(grid, true).expect("2-period bspline torus should succeed");
	let objects = [plasma.color("cyan")];
	let mut f = std::fs::File::create(format!("{example_name}.step")).unwrap();
	cadrum::write_step(&objects, &mut f).unwrap();
	let mut f_svg = std::fs::File::create(format!("{example_name}.svg")).unwrap();
	cadrum::mesh(&objects, 0.1).and_then(|m| m.write_svg(DVec3::new(0.05, 0.05, 1.0), false, true, &mut f_svg)).unwrap();
	eprintln!("wrote {0}.step / {0}.svg", example_name);
}

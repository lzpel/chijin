//! Minimal example: create a box and write it as a STEP file.
//!
//! ```
//! cargo run --example box --features bundled
//! ```
//!
//! Output: out/box.step

use cadrum::Solid;
use glam::DVec3;

fn main() {
	std::fs::create_dir_all("out").unwrap();
	let shape = Solid::box_from_corners(DVec3::ZERO, DVec3::new(10.0, 20.0, 30.0));
	let mut f = std::fs::File::create("out/box.step").expect("failed to create file");
	cadrum::write_step(&[shape], &mut f).expect("failed to write STEP");
	println!("wrote out/box.step");
}

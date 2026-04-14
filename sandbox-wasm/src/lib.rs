pub mod chijin;

use cadrum::Solid;
use glam::DVec3;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn box_svg() -> String {
	let solid = Solid::cube(10.0, 20.0, 30.0).color("#4a90d9");
	cadrum::mesh(&[solid], 0.5).unwrap().to_svg(DVec3::new(1.0, 1.0, 1.0), false).unwrap();
}
pub mod chijin;

use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn chijin_svg() -> String {
	chijin::chijin().to_svg(DVec3::new(1.0, 1.0, 1.0), 0.5).unwrap()
}
//! Integration tests for BRep + color trailer format.

#![cfg(feature = "color")]

use cadrum::{Color, Solid, SolidExt};
use glam::DVec3;
use std::fs;

const COLORED_BOX_STEP: &str = "steps/colored_box.step";

fn read_colored_box() -> Vec<Solid> {
	let data = fs::read(COLORED_BOX_STEP).expect("steps/colored_box.step should exist");
	cadrum::io::read_step(&mut data.as_slice()).expect("read_step should succeed")
}

fn colormap_len(shape: &[Solid]) -> usize {
	shape.iter().map(|s| s.colormap().len()).sum()
}

fn roundtrip_bin(shape: &[Solid]) -> Vec<Solid> {
	let mut buf = Vec::new();
	cadrum::io::write_brep_binary(shape, &mut buf).expect("write_brep_binary should succeed");
	cadrum::io::read_brep_binary(&mut buf.as_slice()).expect("read_brep_binary should succeed")
}

fn roundtrip_text(shape: &[Solid]) -> Vec<Solid> {
	let mut buf = Vec::new();
	cadrum::io::write_brep_text(shape, &mut buf).expect("write_brep_text should succeed");
	cadrum::io::read_brep_text(&mut buf.as_slice()).expect("read_brep_text should succeed")
}

// ── binary tests ─────────────────────────────────────────────────────────────

/// Round-trip (binary) preserves the number of colors and the RGB values.
#[test]
fn bin_write_then_read_preserves_colors() {
	let original = read_colored_box();
	let reloaded = roundtrip_bin(&original);

	assert_eq!(colormap_len(&reloaded), colormap_len(&original), "color count should be preserved (binary)");

	let original_colors: Vec<Color> = original.iter().flat_map(|s| s.face_iter()).filter_map(|f| original.iter().find_map(|s| s.colormap().get(&f.tshape_id()).copied())).collect();
	let reloaded_colors: Vec<Color> = reloaded.iter().flat_map(|s| s.face_iter()).filter_map(|f| reloaded.iter().find_map(|s| s.colormap().get(&f.tshape_id()).copied())).collect();

	assert_eq!(original_colors, reloaded_colors, "RGB values should be identical (binary)");
}

/// A shape with an empty colormap round-trips without error (binary).
#[test]
fn bin_colorless_shape_roundtrip() {
	let shape: Vec<Solid> = vec![Solid::cube(1.0, 1.0, 1.0)];
	let reloaded = roundtrip_bin(&shape);
	assert_eq!(colormap_len(&reloaded), 0);
}

/// Round-trip (binary) after a boolean operation preserves surviving colors.
#[test]
fn bin_roundtrip_after_boolean() {
	let cube = read_colored_box();
	let half: Vec<Solid> = vec![Solid::half_space(DVec3::ZERO, DVec3::NEG_Z)];
	let solids: Vec<Solid> = cube.intersect(&half).expect("intersect should succeed");

	assert!(colormap_len(&solids) >= 1, "at least one color should survive intersect");

	let reloaded = roundtrip_bin(&solids);
	assert_eq!(colormap_len(&reloaded), colormap_len(&solids), "color count should survive round-trip (binary)");
}

// ── text tests ───────────────────────────────────────────────────────────────

/// Round-trip (text) preserves the number of colors and the RGB values.
#[test]
fn text_write_then_read_preserves_colors() {
	let original = read_colored_box();
	let reloaded = roundtrip_text(&original);

	assert_eq!(colormap_len(&reloaded), colormap_len(&original), "color count should be preserved (text)");

	let original_colors: Vec<Color> = original.iter().flat_map(|s| s.face_iter()).filter_map(|f| original.iter().find_map(|s| s.colormap().get(&f.tshape_id()).copied())).collect();
	let reloaded_colors: Vec<Color> = reloaded.iter().flat_map(|s| s.face_iter()).filter_map(|f| reloaded.iter().find_map(|s| s.colormap().get(&f.tshape_id()).copied())).collect();

	assert_eq!(original_colors, reloaded_colors, "RGB values should be identical (text)");
}

/// A shape with an empty colormap round-trips without error (text).
#[test]
fn text_colorless_shape_roundtrip() {
	let shape: Vec<Solid> = vec![Solid::cube(1.0, 1.0, 1.0)];
	let reloaded = roundtrip_text(&shape);
	assert_eq!(colormap_len(&reloaded), 0);
}

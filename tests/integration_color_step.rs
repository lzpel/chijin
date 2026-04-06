//! Integration tests for colored STEP I/O.
//!
//! Reads `steps/colored_box.step` (an AP214 STEP file with per-face colors),
//! applies boolean / clean / translate operations, and writes results to `out/`.

#![cfg(feature = "color")]

use cadrum::Solid;
use glam::DVec3;
use std::fs;

const COLORED_BOX_STEP: &str = "steps/colored_box.step";

/// Read `colored_box.step` and return the shape.  Panics if reading fails.
fn read_colored_box() -> Vec<Solid> {
	let data = fs::read(COLORED_BOX_STEP).expect("steps/colored_box.step should exist");
	cadrum::io::read_step(&mut data.as_slice()).expect("read_step should succeed")
}

fn colormap_len(shape: &[Solid]) -> usize {
	shape.iter().map(|s| s.colormap().len()).sum()
}

// ── helpers ──────────────────────────────────────────────────────────────────

fn write_colored(shape: &[Solid], path: &str) {
	fs::create_dir_all("out").unwrap();
	let mut buf = Vec::new();
	cadrum::io::write_step(shape, &mut buf).expect("write_step should succeed");
	fs::write(path, &buf).expect("should write output file");
}

// ── tests ─────────────────────────────────────────────────────────────────────

/// Reading colored_box.step should yield at least 6 colored faces.
#[test]
fn read_colored_step_populates_colormap() {
	let shape = read_colored_box();
	assert!(
		colormap_len(&shape) >= 6,
		"expected at least 6 colored faces, got {}",
		colormap_len(&shape)
	);
	// Every entry in the colormap should correspond to an actual face.
	let face_ids: std::collections::HashSet<u64> = shape
		.iter()
		.flat_map(|s| s.face_iter())
		.map(|f| f.tshape_id())
		.collect();
	for solid in &shape {
		for id in solid.colormap().keys() {
			assert!(
				face_ids.contains(id),
				"colormap key {:?} does not match any face in the shape",
				id
			);
		}
	}
}

/// Write the colored shape to STEP and read it back — colormap should be
/// non-empty after the round-trip (XDE preserves face colors).
#[test]
fn write_then_read_preserves_colors() {
	let original = read_colored_box();
	let path = "out/colored_box_roundtrip.step";
	write_colored(&original, path);

	let data = fs::read(path).unwrap();
	let reloaded = cadrum::io::read_step(&mut data.as_slice()).expect("re-read should succeed");

	assert!(
		colormap_len(&reloaded) >= 6,
		"re-read shape should have at least 6 colored faces, got {}",
		colormap_len(&reloaded)
	);
}

/// Cut the colored box with a half-space (z > 0) and write the result.
/// The 5 surviving original faces should keep their colors; the new cut face
/// has no color (it comes from the tool which has an empty colormap).
#[test]
fn intersect_colored_step_preserves_colors() {
	let cube = read_colored_box();
	let original_colors = colormap_len(&cube);

	// Half-space keeping z > 0 side.
	let half: Vec<Solid> = vec![Solid::half_space(DVec3::ZERO, DVec3::Z)];
	let result = cadrum::Boolean::intersect(&cube, &half).expect("intersect should succeed");

	// At least one face should have kept its color.
	assert!(
		colormap_len(result.solids()) >= 1,
		"at least one face should keep its color after intersect, got 0"
	);
	assert!(
		colormap_len(result.solids()) < original_colors + 1,
		"intersect should not invent new colors"
	);

	write_colored(result.solids(), "out/colored_box_intersect.step");
}

/// Translate the colored box and verify colors survive the move.
#[test]
fn translate_colored_step_preserves_colors() {
	let shape = read_colored_box();
	let original_len = colormap_len(&shape);

	let moved: Vec<Solid> = shape
		.into_iter()
		.map(|s| s.translate(DVec3::new(100.0, 0.0, 0.0)))
		.collect();

	assert_eq!(
		colormap_len(&moved),
		original_len,
		"translate should preserve all {} face colors",
		original_len
	);
	write_colored(&moved, "out/colored_box_translated.step");
}

/// clean() on the read shape should not lose colors.
#[test]
fn clean_colored_step_preserves_colors() {
	let shape = read_colored_box();
	let original_len = colormap_len(&shape);

	let cleaned: Vec<Solid> = shape
		.iter()
		.map(|s| s.clean().expect("clean should succeed"))
		.collect();

	assert_eq!(
		colormap_len(&cleaned),
		original_len,
		"clean should preserve all {} face colors",
		original_len
	);
	write_colored(&cleaned, "out/colored_box_cleaned.step");
}

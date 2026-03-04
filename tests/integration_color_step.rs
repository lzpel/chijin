//! Integration tests for colored STEP I/O.
//!
//! Reads `steps/colored_box.step` (an AP214 STEP file with per-face colors),
//! applies boolean / clean / translate operations, and writes results to `out/`.

#![cfg(feature = "color")]

use chijin::{Rgb, Shape, TShapeId};
use glam::DVec3;
use std::fs;

const COLORED_BOX_STEP: &str = "steps/colored_box.step";

/// Read `colored_box.step` and return the shape.  Panics if reading fails.
fn read_colored_box() -> Shape {
    let data = fs::read(COLORED_BOX_STEP)
        .expect("steps/colored_box.step should exist");
    Shape::read_step_with_colors(&mut data.as_slice())
        .expect("read_step_with_colors should succeed")
}

// ── helpers ──────────────────────────────────────────────────────────────────

fn write_colored(shape: &Shape, path: &str) {
    fs::create_dir_all("out").unwrap();
    let mut buf = Vec::new();
    shape
        .write_step_with_colors(&mut buf)
        .expect("write_step_with_colors should succeed");
    fs::write(path, &buf).expect("should write output file");
}

// ── tests ─────────────────────────────────────────────────────────────────────

/// Reading colored_box.step should yield at least 6 colored faces.
#[test]
fn read_colored_step_populates_colormap() {
    let shape = read_colored_box();
    assert!(
        shape.colormap.len() >= 6,
        "expected at least 6 colored faces, got {}",
        shape.colormap.len()
    );
    // Every entry in the colormap should correspond to an actual face.
    let face_ids: std::collections::HashSet<TShapeId> =
        shape.faces().map(|f| f.tshape_id()).collect();
    for id in shape.colormap.keys() {
        assert!(
            face_ids.contains(id),
            "colormap key {:?} does not match any face in the shape",
            id
        );
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
    let reloaded = Shape::read_step_with_colors(&mut data.as_slice())
        .expect("re-read should succeed");

    assert!(
        reloaded.colormap.len() >= 6,
        "re-read shape should have at least 6 colored faces, got {}",
        reloaded.colormap.len()
    );
}

/// Cut the colored box with a half-space (z > 0) and write the result.
/// The 5 surviving original faces should keep their colors; the new cut face
/// has no color (it comes from the tool which has an empty colormap).
#[test]
fn intersect_colored_step_preserves_colors() {
    let cube = read_colored_box();
    let original_colors = cube.colormap.len();

    // Half-space keeping z > 0 side.
    let half = Shape::half_space(DVec3::ZERO, DVec3::Z);
    let result = cube.intersect(&half).expect("intersect should succeed");

    // At least one face should have kept its color.
    assert!(
        result.shape.colormap.len() >= 1,
        "at least one face should keep its color after intersect, got 0"
    );
    assert!(
        result.shape.colormap.len() < original_colors + 1,
        "intersect should not invent new colors"
    );

    write_colored(&result.shape, "out/colored_box_intersect.step");
}

/// Translate the colored box and verify colors survive the move.
#[test]
fn translate_colored_step_preserves_colors() {
    let shape = read_colored_box();
    let original_len = shape.colormap.len();

    let moved = shape.translated(DVec3::new(100.0, 0.0, 0.0));

    assert_eq!(
        moved.colormap.len(),
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
    let original_len = shape.colormap.len();

    let cleaned = shape.clean().expect("clean should succeed");

    assert_eq!(
        cleaned.colormap.len(),
        original_len,
        "clean should preserve all {} face colors",
        original_len
    );
    write_colored(&cleaned, "out/colored_box_cleaned.step");
}

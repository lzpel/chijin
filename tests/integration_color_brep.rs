//! Integration tests for the CHJC (BRep + color) binary format.

#![cfg(feature = "color")]

use chijin::{Rgb, Shape, TShapeId};
use glam::DVec3;
use std::fs;

const COLORED_BOX_STEP: &str = "steps/colored_box.step";

fn read_colored_box() -> Shape {
    let data = fs::read(COLORED_BOX_STEP).expect("steps/colored_box.step should exist");
    Shape::read_step_with_colors(&mut data.as_slice())
        .expect("read_step_with_colors should succeed")
}

fn roundtrip(shape: &Shape) -> Shape {
    let mut buf = Vec::new();
    shape
        .write_brep_color(&mut buf)
        .expect("write_brep_color should succeed");
    Shape::read_brep_color(&mut buf.as_slice()).expect("read_brep_color should succeed")
}

// ── tests ─────────────────────────────────────────────────────────────────────

/// Round-trip preserves the number of colors and the RGB values.
#[test]
fn write_then_read_preserves_colors() {
    let original = read_colored_box();
    let reloaded = roundtrip(&original);

    assert_eq!(
        reloaded.colormap.len(),
        original.colormap.len(),
        "color count should be preserved"
    );

    // Collect original colors by face traversal index so we can compare
    // after TShapeId changes on reload.
    let original_colors: Vec<Rgb> = original
        .faces()
        .filter_map(|f| original.colormap.get(&f.tshape_id()).copied())
        .collect();
    let reloaded_colors: Vec<Rgb> = reloaded
        .faces()
        .filter_map(|f| reloaded.colormap.get(&f.tshape_id()).copied())
        .collect();

    assert_eq!(original_colors, reloaded_colors, "RGB values should be identical");
}

/// A shape with an empty colormap round-trips without error.
#[test]
fn colorless_shape_roundtrip() {
    let shape = Shape::box_from_corners(DVec3::ZERO, DVec3::ONE);
    let reloaded = roundtrip(&shape);
    assert_eq!(reloaded.colormap.len(), 0);
}

/// Round-trip after a boolean operation preserves the surviving colors.
#[test]
fn roundtrip_after_boolean() {
    let cube = read_colored_box();
    let half = Shape::half_space(DVec3::ZERO, DVec3::NEG_Z);
    let cut = cube.intersect(&half).expect("intersect should succeed");

    assert!(cut.shape.colormap.len() >= 1, "at least one color should survive intersect");

    let reloaded = roundtrip(&cut.shape);
    assert_eq!(
        reloaded.colormap.len(),
        cut.shape.colormap.len(),
        "color count should survive round-trip"
    );
}

/// Invalid magic bytes return BrepReadFailed.
#[test]
fn invalid_magic_returns_error() {
    let bad = b"XXXX\x01\x00\x00\x00\x00";
    let result = Shape::read_brep_color(&mut bad.as_slice());
    assert!(result.is_err());
}

/// Wrong version byte returns BrepReadFailed.
#[test]
fn wrong_version_returns_error() {
    let bad = b"CHJC\x02\x00\x00\x00\x00";
    let result = Shape::read_brep_color(&mut bad.as_slice());
    assert!(result.is_err());
}

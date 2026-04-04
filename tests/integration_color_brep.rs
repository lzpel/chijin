//! Integration tests for the CHJC (BRep + color) binary format.

#![cfg(feature = "color")]

use cadrum::{Color, SolidTrait, Solid};
use glam::DVec3;
use std::fs;

const COLORED_BOX_STEP: &str = "steps/colored_box.step";

fn read_colored_box() -> Vec<Solid> {
    let data = fs::read(COLORED_BOX_STEP).expect("steps/colored_box.step should exist");
    cadrum::read_step_with_colors(&mut data.as_slice())
        .expect("read_step_with_colors should succeed")
}

fn colormap_len(shape: &[Solid]) -> usize {
    shape.iter().map(|s| s.colormap().len()).sum()
}

fn roundtrip(shape: &[Solid]) -> Vec<Solid> {
    let mut buf = Vec::new();
    cadrum::write_brep_color(shape, &mut buf)
        .expect("write_brep_color should succeed");
    cadrum::read_brep_color(&mut buf.as_slice()).expect("read_brep_color should succeed")
}

// ── tests ─────────────────────────────────────────────────────────────────────

/// Round-trip preserves the number of colors and the RGB values.
#[test]
fn write_then_read_preserves_colors() {
    let original = read_colored_box();
    let reloaded = roundtrip(&original);

    assert_eq!(
        colormap_len(&reloaded),
        colormap_len(&original),
        "color count should be preserved"
    );

    // Collect original colors by face traversal index so we can compare
    // after TShapeId changes on reload.
    let original_colors: Vec<Color> = original
        .iter().flat_map(|s| s.face_iter())
        .filter_map(|f| original.iter().find_map(|s| s.colormap().get(&f.tshape_id()).copied()))
        .collect();
    let reloaded_colors: Vec<Color> = reloaded
        .iter().flat_map(|s| s.face_iter())
        .filter_map(|f| reloaded.iter().find_map(|s| s.colormap().get(&f.tshape_id()).copied()))
        .collect();

    assert_eq!(original_colors, reloaded_colors, "RGB values should be identical");
}

/// A shape with an empty colormap round-trips without error.
#[test]
fn colorless_shape_roundtrip() {
    let shape: Vec<Solid> = vec![Solid::box_from_corners(DVec3::ZERO, DVec3::ONE)];
    let reloaded = roundtrip(&shape);
    assert_eq!(colormap_len(&reloaded), 0);
}

/// Round-trip after a boolean operation preserves the surviving colors.
#[test]
fn roundtrip_after_boolean() {
    let cube = read_colored_box();
    let half: Vec<Solid> = vec![Solid::half_space(DVec3::ZERO, DVec3::NEG_Z)];
    let cut = cadrum::Boolean::intersect(&cube, &half).expect("intersect should succeed");

    assert!(colormap_len(&cut.solids) >= 1, "at least one color should survive intersect");

    let reloaded = roundtrip(&cut.solids);
    assert_eq!(
        colormap_len(&reloaded),
        colormap_len(&cut.solids),
        "color count should survive round-trip"
    );
}

/// Invalid magic bytes return BrepReadFailed.
#[test]
fn invalid_magic_returns_error() {
    let bad = b"XXXX\x01\x00\x00\x00\x00";
    let result = cadrum::read_brep_color(&mut bad.as_slice());
    assert!(result.is_err());
}

/// Wrong version byte returns BrepReadFailed.
#[test]
fn wrong_version_returns_error() {
    let bad = b"CHJC\x02\x00\x00\x00\x00";
    let result = cadrum::read_brep_color(&mut bad.as_slice());
    assert!(result.is_err());
}

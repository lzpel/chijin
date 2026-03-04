//! Integration tests for the `color` feature.
//!
//! Tests that Boolean operations correctly propagate face colors:
//! - Unchanged faces keep their color.
//! - Trimmed (modified) faces keep their color.
//! - Deleted faces are removed from the colormap.
//! - Newly created cross-section faces have no color.

#![cfg(feature = "color")]

use chijin::{Rgb, Shape, TShapeId};
use glam::DVec3;

/// Assign a distinct color to every face of `shape` based on its outward normal.
/// Returns the number of faces that were colored (should equal the total face count).
fn color_box_faces(shape: &mut Shape) -> usize {
    // (direction, color) pairs — one per axis side
    let palette: &[(DVec3, Rgb)] = &[
        (DVec3::Z,     Rgb { r: 1.0, g: 0.0, b: 0.0 }), // top    (+Z): red
        (DVec3::NEG_Z, Rgb { r: 0.0, g: 0.0, b: 1.0 }), // bottom (-Z): blue
        (DVec3::Y,     Rgb { r: 0.0, g: 1.0, b: 0.0 }), // back   (+Y): green
        (DVec3::NEG_Y, Rgb { r: 1.0, g: 1.0, b: 0.0 }), // front  (-Y): yellow
        (DVec3::X,     Rgb { r: 0.0, g: 1.0, b: 1.0 }), // right  (+X): cyan
        (DVec3::NEG_X, Rgb { r: 1.0, g: 0.0, b: 1.0 }), // left   (-X): magenta
    ];

    let mut count = 0;
    // Collect (id, normal) pairs first so we don't borrow shape.colormap while iterating.
    let id_normal: Vec<(TShapeId, DVec3)> = shape
        .faces()
        .map(|f| (f.tshape_id(), f.normal_at_center()))
        .collect();

    for (id, normal) in id_normal {
        for (dir, color) in palette {
            if normal.dot(*dir) > 0.9 {
                shape.colormap.insert(id, *color);
                count += 1;
                break;
            }
        }
    }
    count
}

/// 2×2×2 box (−1..1 on each axis), z > 0 half-space intersect.
///
/// Expected geometry after intersect:
///   shape     → 5 faces: top + 4 trimmed sides (bottom deleted)
///   new_faces → 1 face: z=0 cross-section
///
/// Expected colors:
///   shape.colormap has 5 entries (top=red, 4 sides with original side colors)
///   new_faces.colormap is empty (cut face is new)
#[test]
fn colored_box_intersect_z_positive_half_space() {
    // ── Build colored box ────────────────────────────────────────────────────
    let mut cube = Shape::box_from_corners(DVec3::splat(-1.0), DVec3::splat(1.0));
    let colored = color_box_faces(&mut cube);
    assert_eq!(colored, 6, "all 6 faces of the box should receive a color");
    assert_eq!(cube.colormap.len(), 6);

    // ── Intersect with half-space z > 0 ─────────────────────────────────────
    // half_space(origin=(0,0,0), normal=(0,0,1)) keeps the z > 0 region.
    let half = Shape::half_space(DVec3::ZERO, DVec3::Z);
    let result = cube.intersect(&half).expect("intersect should succeed");

    // ── Topology checks ──────────────────────────────────────────────────────
    // The closed solid has 6 faces: top + 4 trimmed sides + z=0 cross-section.
    // new_faces is an additional copy of the z=0 face for downstream use.
    let shape_face_count = result.shape.faces().count();
    let new_face_count = result.new_faces.faces().count();
    assert_eq!(shape_face_count, 6, "result.shape should have 6 faces (top + 4 sides + cut)");
    assert_eq!(new_face_count, 1, "result.new_faces should have 1 cross-section face");

    // ── Colormap size ────────────────────────────────────────────────────────
    // 5 faces from the original box carry a color; the z=0 cut face (from half_space,
    // which has an empty colormap) gets no color.
    assert_eq!(
        result.shape.colormap.len(),
        5,
        "5 faces (top + 4 trimmed sides) should carry a color; cut face has none"
    );
    assert_eq!(
        result.new_faces.colormap.len(),
        0,
        "the new cross-section face should have no color"
    );

    // ── Top face (normal ≈ +Z) should be red ─────────────────────────────────
    let top = result
        .shape
        .faces()
        .find(|f| f.normal_at_center().dot(DVec3::Z) > 0.9)
        .expect("top face (+Z) should exist in result");
    let top_color = result
        .shape
        .colormap
        .get(&top.tshape_id())
        .expect("top face should have a color");
    assert!(
        (top_color.r - 1.0).abs() < 1e-6 && top_color.g < 1e-6 && top_color.b < 1e-6,
        "top face should be red, got {:?}",
        top_color
    );

    // ── Right face (normal ≈ +X, trimmed) should be cyan ─────────────────────
    // This face is trimmed by the boolean op: its TShape* changed, but
    // from_a mapping ensures the original cyan color is preserved (修正案2).
    let right = result
        .shape
        .faces()
        .find(|f| f.normal_at_center().dot(DVec3::X) > 0.9)
        .expect("right face (+X) should exist in result");
    let right_color = result
        .shape
        .colormap
        .get(&right.tshape_id())
        .expect("right face should have a color (trimmed face color must be preserved)");
    assert!(
        right_color.r < 1e-6
            && (right_color.g - 1.0).abs() < 1e-6
            && (right_color.b - 1.0).abs() < 1e-6,
        "right face (+X) should be cyan, got {:?}",
        right_color
    );

    // ── Bottom face (normal ≈ −Z, center at z ≈ −1) must NOT appear ──────────
    // The bottom face is deleted by the intersect; it should not exist.
    // Note: the z=0 cut face also has normal ≈ -Z, so we check center.z as well.
    let bottom_in_result = result
        .shape
        .faces()
        .any(|f| f.normal_at_center().dot(DVec3::NEG_Z) > 0.9 && f.center_of_mass().z < -0.5);
    assert!(!bottom_in_result, "bottom face (-Z) at z=-1 should be deleted by intersect");
}

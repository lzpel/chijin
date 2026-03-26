//! Integration tests for the `color` feature.
//!
//! Tests that Boolean operations correctly propagate face colors:
//! - Unchanged faces keep their color.
//! - Trimmed (modified) faces keep their color.
//! - Deleted faces are removed from the colormap.
//! - Newly created cross-section faces have no color.

#![cfg(feature = "color")]

use chijin::{Rgb, Shape, Solid, TShapeId};
use glam::DVec3;

/// Assign a distinct color to every face of `shape` based on its outward normal.
/// Returns the number of faces that were colored (should equal the total face count).
fn color_box_faces(shape: &mut Vec<Solid>) -> usize {
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
    // Collect (id, normal) pairs first so we don't borrow shape while iterating.
    let id_normal: Vec<(TShapeId, DVec3)> = shape
        .faces()
        .map(|f| (f.tshape_id(), f.normal_at_center()))
        .collect();

    for (id, normal) in id_normal {
        for (dir, color) in palette {
            if normal.dot(*dir) > 0.9 {
                shape[0].colormap_mut().insert(id, *color);
                count += 1;
                break;
            }
        }
    }
    count
}

/// Helper: count total colormap entries across all solids.
fn colormap_len(shape: &[Solid]) -> usize {
    shape.iter().map(|s| s.colormap().len()).sum()
}

/// Helper: check if a TShapeId has a color in any solid's colormap.
fn colormap_contains(shape: &[Solid], id: &TShapeId) -> bool {
    shape.iter().any(|s| s.colormap().contains_key(id))
}

/// Helper: get color for a TShapeId from any solid's colormap.
fn colormap_get(shape: &[Solid], id: &TShapeId) -> Option<Rgb> {
    shape.iter().find_map(|s| s.colormap().get(id).copied())
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
    let mut cube: Vec<Solid> = vec![Solid::box_from_corners(DVec3::splat(-1.0), DVec3::splat(1.0))];
    let colored = color_box_faces(&mut cube);
    assert_eq!(colored, 6, "all 6 faces of the box should receive a color");
    assert_eq!(cube[0].colormap().len(), 6);

    // ── Intersect with half-space z > 0 ─────────────────────────────────────
    // half_space(origin=(0,0,0), normal=(0,0,1)) keeps the z > 0 region.
    let half: Vec<Solid> = vec![Solid::half_space(DVec3::ZERO, DVec3::Z)];
    let result = chijin::Boolean::intersect(&cube, &half).expect("intersect should succeed");

    // ── Topology checks ──────────────────────────────────────────────────────
    // The closed solid has 6 faces: top + 4 trimmed sides + z=0 cross-section.
    // is_tool_face identifies the cross-section face(s) from the tool (half-space).
    let shape_face_count = result.solids.faces().count();
    let tool_face_count = result.solids.faces().filter(|f| result.is_tool_face(f)).count();
    assert_eq!(shape_face_count, 6, "result.solids should have 6 faces (top + 4 sides + cut)");
    assert_eq!(tool_face_count, 1, "should have 1 tool (cross-section) face");

    // ── Colormap size ────────────────────────────────────────────────────────
    // 5 faces from the original box carry a color; the z=0 cut face (from half_space,
    // which has an empty colormap) gets no color.
    assert_eq!(
        colormap_len(&result.solids),
        5,
        "5 faces (top + 4 trimmed sides) should carry a color; cut face has none"
    );
    // Tool faces should have no color (half-space has empty colormap).
    for f in result.solids.faces().filter(|f| result.is_tool_face(f)) {
        assert!(
            !colormap_contains(&result.solids, &f.tshape_id()),
            "tool face should have no color"
        );
    }

    // ── Top face (normal ≈ +Z) should be red ─────────────────────────────────
    let top = result
        .solids
        .faces()
        .find(|f| f.normal_at_center().dot(DVec3::Z) > 0.9)
        .expect("top face (+Z) should exist in result");
    let top_color = colormap_get(&result.solids, &top.tshape_id())
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
        .solids
        .faces()
        .find(|f| f.normal_at_center().dot(DVec3::X) > 0.9)
        .expect("right face (+X) should exist in result");
    let right_color = colormap_get(&result.solids, &right.tshape_id())
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
        .solids
        .faces()
        .any(|f| f.normal_at_center().dot(DVec3::NEG_Z) > 0.9 && f.center_of_mass().z < -0.5);
    assert!(!bottom_in_result, "bottom face (-Z) at z=-1 should be deleted by intersect");
}

/// Verify that `Shape::clean()` preserves face colors.
///
/// Strategy: build a colored box, call clean(), and assert every face in the
/// cleaned result still carries a color.  A plain box already has
/// clean topology, so `ShapeUpgrade_UnifySameDomain` will emit an identity
/// mapping (new_id == old_id for every face) — the simplest possible path
/// through the color-remapping code.
#[test]
fn clean_preserves_face_colors() {
    let mut cube: Vec<Solid> = vec![Solid::box_from_corners(DVec3::splat(-1.0), DVec3::splat(1.0))];
    let colored = color_box_faces(&mut cube);
    assert_eq!(colored, 6);

    let cleaned = cube.clean().expect("clean should succeed");

    // Every face in the cleaned shape must have a color.
    let mut colored_after = 0usize;
    for f in cleaned.faces() {
        assert!(
            colormap_contains(&cleaned, &f.tshape_id()),
            "face {:?} lost its color after clean",
            f.tshape_id()
        );
        colored_after += 1;
    }
    assert_eq!(colored_after, 6, "cleaned box should still have 6 colored faces");
}

/// Verify that clean() preserves colors when two adjacent same-plane faces
/// are unified into one.
///
/// Two unit boxes share the face at x = 1.  After union the internal wall
/// disappears; the top / bottom / front / back faces are each split into two
/// coplanar patches that `clean()` merges into one.  The merged face must
/// carry a color (the one from whichever original patch is visited first).
#[test]
fn clean_merge_preserves_color() {
    // Box A: x ∈ [0,1], y ∈ [0,1], z ∈ [0,1]
    let mut a: Vec<Solid> = vec![Solid::box_from_corners(DVec3::new(0.0, 0.0, 0.0), DVec3::new(1.0, 1.0, 1.0))];
    color_box_faces(&mut a);

    // Box B: x ∈ [1,2], y ∈ [0,1], z ∈ [0,1]  (adjacent, sharing the x=1 face)
    let mut b: Vec<Solid> = vec![Solid::box_from_corners(DVec3::new(1.0, 0.0, 0.0), DVec3::new(2.0, 1.0, 1.0))];
    color_box_faces(&mut b);

    // Union produces a 2×1×1 slab whose side faces may be split at x=1.
    let unioned: Vec<Solid> = chijin::Boolean::union(&a, &b).expect("union should succeed").into();

    // clean() merges coplanar adjacent patches.
    let cleaned = unioned.clean().expect("clean should succeed");

    // Every face in the cleaned shape must have a color.
    for f in cleaned.faces() {
        assert!(
            colormap_contains(&cleaned, &f.tshape_id()),
            "face {:?} lost its color after clean+merge",
            f.tshape_id()
        );
    }
    // The 2×1×1 slab has 6 faces after clean.
    let face_count = cleaned.faces().count();
    assert_eq!(face_count, 6, "cleaned slab should have 6 faces, got {}", face_count);
    assert_eq!(
        colormap_len(&cleaned),
        6,
        "all 6 faces should carry a color after clean"
    );
}

//! Integration tests for the `color` feature.
//!
//! Tests that Boolean operations correctly propagate face colors:
//! - Unchanged faces keep their color.
//! - Trimmed (modified) faces keep their color.
//! - Deleted faces are removed from the colormap.
//! - Newly created cross-section faces have no color.

#![cfg(feature = "color")]

use cadrum::{Color, Solid, Transform, SolidExt};
use glam::DVec3;

/// Assign a distinct color to every face of `shape` based on its outward normal.
/// Returns the number of faces that were colored (should equal the total face count).
fn color_box_faces(shape: &mut Vec<Solid>) -> usize {
	// (direction, color) pairs — one per axis side
	let palette: &[(DVec3, Color)] = &[
		(DVec3::Z, Color { r: 1.0, g: 0.0, b: 0.0 }),     // top    (+Z): red
		(DVec3::NEG_Z, Color { r: 0.0, g: 0.0, b: 1.0 }), // bottom (-Z): blue
		(DVec3::Y, Color { r: 0.0, g: 1.0, b: 0.0 }),     // back   (+Y): green
		(DVec3::NEG_Y, Color { r: 1.0, g: 1.0, b: 0.0 }), // front  (-Y): yellow
		(DVec3::X, Color { r: 0.0, g: 1.0, b: 1.0 }),     // right  (+X): cyan
		(DVec3::NEG_X, Color { r: 1.0, g: 0.0, b: 1.0 }), // left   (-X): magenta
	];

	let mut count = 0;
	// Collect (id, normal) pairs first so we don't borrow shape while iterating.
	let id_normal: Vec<(u64, DVec3)> = shape.iter().flat_map(|s| s.face_iter()).map(|f| (f.tshape_id(), f.normal_at_center())).collect();

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

/// Helper: check if a u64 has a color in any solid's colormap.
fn colormap_contains(shape: &[Solid], id: &u64) -> bool {
	shape.iter().any(|s| s.colormap().contains_key(id))
}

/// Helper: get color for a u64 from any solid's colormap.
fn colormap_get(shape: &[Solid], id: &u64) -> Option<Color> {
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
	let mut cube: Vec<Solid> = vec![Solid::cube(2.0, 2.0, 2.0).translate(DVec3::splat(-1.0))];
	let colored = color_box_faces(&mut cube);
	assert_eq!(colored, 6, "all 6 faces of the box should receive a color");
	assert_eq!(cube[0].colormap().len(), 6);

	// ── Intersect with half-space z > 0 ─────────────────────────────────────
	// half_space(origin=(0,0,0), normal=(0,0,1)) keeps the z > 0 region.
	let half: Vec<Solid> = vec![Solid::half_space(DVec3::ZERO, DVec3::Z)];
	let (solids, meta) = cube.intersect_with_metadata(&half).expect("intersect should succeed");

	// ── Topology checks ──────────────────────────────────────────────────────
	// The closed solid has 6 faces: top + 4 trimmed sides + z=0 cross-section.
	// is_tool_face identifies the cross-section face(s) from the tool (half-space).
	let shape_face_count = solids.iter().flat_map(|s| s.face_iter()).count();
	let tool_face_count = solids.iter().flat_map(|s| s.face_iter()).filter(|f| cadrum::is_tool_face(&meta, f)).count();
	assert_eq!(shape_face_count, 6, "result should have 6 faces (top + 4 sides + cut)");
	assert_eq!(tool_face_count, 1, "should have 1 tool (cross-section) face");

	// ── Colormap size ────────────────────────────────────────────────────────
	// 5 faces from the original box carry a color; the z=0 cut face (from half_space,
	// which has an empty colormap) gets no color.
	assert_eq!(colormap_len(&solids), 5, "5 faces (top + 4 trimmed sides) should carry a color; cut face has none");
	// Tool faces should have no color (half-space has empty colormap).
	for f in solids.iter().flat_map(|s| s.face_iter()).filter(|f| cadrum::is_tool_face(&meta, f)) {
		assert!(!colormap_contains(&solids, &f.tshape_id()), "tool face should have no color");
	}

	// ── Top face (normal ≈ +Z) should be red ─────────────────────────────────
	let top = solids.iter().flat_map(|s| s.face_iter()).find(|f| f.normal_at_center().dot(DVec3::Z) > 0.9).expect("top face (+Z) should exist in result");
	let top_color = colormap_get(&solids, &top.tshape_id()).expect("top face should have a color");
	assert!((top_color.r - 1.0).abs() < 1e-6 && top_color.g < 1e-6 && top_color.b < 1e-6, "top face should be red, got {:?}", top_color);

	// ── Right face (normal ≈ +X, trimmed) should be cyan ─────────────────────
	// This face is trimmed by the boolean op: its TShape* changed, but
	// from_a mapping ensures the original cyan color is preserved (修正案2).
	let right = solids.iter().flat_map(|s| s.face_iter()).find(|f| f.normal_at_center().dot(DVec3::X) > 0.9).expect("right face (+X) should exist in result");
	let right_color = colormap_get(&solids, &right.tshape_id()).expect("right face should have a color (trimmed face color must be preserved)");
	assert!(right_color.r < 1e-6 && (right_color.g - 1.0).abs() < 1e-6 && (right_color.b - 1.0).abs() < 1e-6, "right face (+X) should be cyan, got {:?}", right_color);

	// ── Bottom face (normal ≈ −Z, center at z ≈ −1) must NOT appear ──────────
	// The bottom face is deleted by the intersect; it should not exist.
	// Note: the z=0 cut face also has normal ≈ -Z, so we check center.z as well.
	let bottom_in_result = solids.iter().flat_map(|s| s.face_iter()).any(|f| f.normal_at_center().dot(DVec3::NEG_Z) > 0.9 && f.center_of_mass().z < -0.5);
	assert!(!bottom_in_result, "bottom face (-Z) at z=-1 should be deleted by intersect");
}

/// 読み書きのラウンドトリップで色が保存されるか検証する。
///
/// examples/02_write_read.rs と同じ手順:
///   0. steps/colored_box.step を読み込む
///   1. STEP ラウンドトリップ (30°回転 → 書き出し → 読み込み)
///   2. BRep text ラウンドトリップ (30°回転 → 書き出し → 読み込み)
///   3. BRep binary ラウンドトリップ (30°回転 → 書き出し → 読み込み)
///
/// 各段階で「colormapが空でないこと」「各面の色が書き出し前と一致すること」を検証する。
#[test]
fn colored_box_step_brep_roundtrips() {
	/// 各面の色を法線方向ラベル("+X","-Z"等)をキーにして取得する。
	/// 回転後も法線の軸成分で面を識別できるようにする。
	fn color_snapshot(solids: &[Solid]) -> std::collections::HashMap<String, Color> {
		let axes: &[(DVec3, &str)] = &[
			(DVec3::X, "+X"), (DVec3::NEG_X, "-X"),
			(DVec3::Y, "+Y"), (DVec3::NEG_Y, "-Y"),
			(DVec3::Z, "+Z"), (DVec3::NEG_Z, "-Z"),
		];
		let mut map = std::collections::HashMap::new();
		for s in solids {
			for f in s.face_iter() {
				if let Some(&c) = s.colormap().get(&f.tshape_id()) {
					let n = f.normal_at_center();
					for &(axis, label) in axes {
						if n.dot(axis) > 0.9 {
							map.insert(label.to_string(), c);
							break;
						}
					}
				}
			}
		}
		map
	}

	/// 書き出し前後の色スナップショットを比較し、不一致があればパニックする。
	fn assert_colors_match(label: &str, before: &std::collections::HashMap<String, Color>, after: &std::collections::HashMap<String, Color>) {
		assert!(!after.is_empty(), "{label}: ラウンドトリップ後にcolormapが空になった");
		assert_eq!(before.len(), after.len(), "{label}: 色付き面の数が変わった ({} → {})", before.len(), after.len());
		for (axis, c_before) in before {
			let c_after = after.get(axis).unwrap_or_else(|| panic!("{label}: {axis} 面の色が失われた"));
			assert!((c_before.r - c_after.r).abs() < 0.02
				&& (c_before.g - c_after.g).abs() < 0.02
				&& (c_before.b - c_after.b).abs() < 0.02,
				"{label}: {axis} 面の色が不一致: {:?} → {:?}", c_before, c_after);
		}
	}

	// 0. colored_box.step を読み込む
	let manifest_dir = env!("CARGO_MANIFEST_DIR");
	let original = cadrum::io::read_step(
		&mut std::fs::File::open(format!("{manifest_dir}/steps/colored_box.step")).expect("open colored_box.step"),
	).expect("read_step");
	let snap0 = color_snapshot(&original);
	assert!(!snap0.is_empty(), "colored_box.step に色付き面が含まれていない");

	// 1. STEP ラウンドトリップ: 書き出し → 読み戻し
	let mut buf = Vec::new();
	cadrum::io::write_step(&original, &mut buf).expect("write_step");
	let a = cadrum::io::read_step(&mut std::io::Cursor::new(&buf)).expect("read_step round-trip");
	let snap_a = color_snapshot(&a);

	// 2. BRep text ラウンドトリップ: 書き出し → 読み戻し
	let mut buf = Vec::new();
	cadrum::io::write_brep_text(&original, &mut buf).expect("write_brep_text");
	let b = cadrum::io::read_brep_text(&mut std::io::Cursor::new(&buf)).expect("read_brep_text round-trip");
	let snap_b = color_snapshot(&b);

	// 3. BRep binary ラウンドトリップ: 書き出し → 読み戻し
	let mut buf = Vec::new();
	cadrum::io::write_brep_binary(&original, &mut buf).expect("write_brep_binary");
	let c = cadrum::io::read_brep_binary(&mut std::io::Cursor::new(&buf)).expect("read_brep_binary round-trip");
	let snap_c = color_snapshot(&c);

	// 4. 目視確認用: original, STEP, BRep text, BRep binary を横に並べてSTL出力
	let [min, max] = original[0].bounding_box();
	let spacing = (max - min).length() * 1.5;
	let all: Vec<Solid> = [original, a, b, c].into_iter()
		.enumerate()
		.flat_map(|(i, solids)| solids.translate(DVec3::X * spacing * i as f64))
		.collect();
	let mut stl = std::fs::File::create(format!("{manifest_dir}/target/colored_box_roundtrip.stl")).expect("create stl");
	cadrum::io::write_stl(&all, 0.1, &mut stl).expect("write_stl");
	eprintln!("STL出力: target/colored_box_roundtrip.stl");

	// 5. 色の比較 (STL出力後にassertする)
	assert_colors_match("STEP", &snap0, &snap_a);
	assert_colors_match("BRep text", &snap0, &snap_b);
	assert_colors_match("BRep binary", &snap0, &snap_c);
}

/// STL/SVG出力で色が変換されず維持されることを検証する。
///
/// 1. 6面に既知の色を付けたboxをメッシュ化
/// 2. STL出力 → attribute bytesのRGB555をデコードして元の色と比較 (5bit精度)
/// 3. SVG出力 → rgb(R,G,B)をパースして元の色と比較 (8bit精度)
#[test]
fn stl_svg_preserve_colors() {
	let mut cube: Vec<Solid> = vec![Solid::cube(2.0, 2.0, 2.0).translate(DVec3::splat(-1.0))];
	color_box_faces(&mut cube);

	// 元の色をface_id→Colorで取得
	let original_colors: std::collections::HashMap<u64, Color> = cube[0].colormap().clone();
	assert_eq!(original_colors.len(), 6);

	// メッシュ化
	let mesh = cadrum::io::mesh(&cube, 0.1).expect("mesh");

	// --- STL検証 ---
	// バイナリSTLをメモリに書き出し、ファイルにも保存
	let mut stl_buf = Vec::new();
	mesh.write_stl(&mut stl_buf).expect("write_stl");
	let manifest_dir = env!("CARGO_MANIFEST_DIR");
	std::fs::write(format!("{manifest_dir}/target/color_box.stl"), &stl_buf).expect("write stl file");
	eprintln!("STL出力: target/color_box.stl");

	// パース: 80バイトヘッダ + 4バイト三角形数 + 三角形ごとに50バイト
	let tri_count = u32::from_le_bytes(stl_buf[80..84].try_into().unwrap()) as usize;
	assert!(tri_count > 0);

	for ti in 0..tri_count {
		let base = 84 + ti * 50;
		let attr = u16::from_le_bytes(stl_buf[base + 48..base + 50].try_into().unwrap());

		let face_id = mesh.face_ids[ti];
		if let Some(orig) = original_colors.get(&face_id) {
			// 色付き面: attribute bytesにRGB555が書かれているはず
			assert!(attr & 0x8000 != 0, "三角形{ti}: 色付き面なのにvalid bitが立っていない");
			let r5 = (attr & 0x1F) as f32;
			let g5 = ((attr >> 5) & 0x1F) as f32;
			let b5 = ((attr >> 10) & 0x1F) as f32;
			// 5bit精度: 31段階なので許容誤差は 1/31 ≈ 0.033
			let tol = 1.1 / 31.0;
			assert!((orig.r - r5 / 31.0).abs() < tol, "STL 三角形{ti} R: {:.3} → {:.3}", orig.r, r5 / 31.0);
			assert!((orig.g - g5 / 31.0).abs() < tol, "STL 三角形{ti} G: {:.3} → {:.3}", orig.g, g5 / 31.0);
			assert!((orig.b - b5 / 31.0).abs() < tol, "STL 三角形{ti} B: {:.3} → {:.3}", orig.b, b5 / 31.0);
		} else {
			// 色なし面: attribute bytesは0
			assert_eq!(attr, 0, "三角形{ti}: 色なし面なのにattributeが非ゼロ");
		}
	}

	// --- SVG検証 ---
	let svg = mesh.to_svg(DVec3::new(1.0, 1.0, 2.0));
	std::fs::write(format!("{manifest_dir}/target/color_box.svg"), &svg).expect("write svg file");
	eprintln!("SVG出力: target/color_box.svg");

	// 元の色をrgb(R,G,B)文字列に変換して集合にする
	let expected_rgbs: std::collections::HashSet<String> = original_colors.values()
		.map(|c| format!("rgb({},{},{})", (c.r * 255.0) as u8, (c.g * 255.0) as u8, (c.b * 255.0) as u8))
		.collect();

	// SVGからfill="rgb(...)"を抽出
	let mut found_rgbs: std::collections::HashSet<String> = std::collections::HashSet::new();
	for segment in svg.split("fill=\"") {
		if segment.starts_with("rgb(") {
			if let Some(end) = segment.find('"') {
				found_rgbs.insert(segment[..end].to_string());
			}
		}
	}

	// SVG内の全色が元の色のいずれかと一致すること（変換されていない）
	// 注: カメラから見える面だけが描画されるので全6色は出ないが、出た色は正確であるべき
	assert!(!found_rgbs.is_empty(), "SVGにrgb色が1つも見つからない");
	for found in &found_rgbs {
		assert!(expected_rgbs.contains(found), "SVGに未知の色 {} がある。期待される色: {:?}", found, expected_rgbs);
	}
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
	let mut cube: Vec<Solid> = vec![Solid::cube(2.0, 2.0, 2.0).translate(DVec3::splat(-1.0))];
	let colored = color_box_faces(&mut cube);
	assert_eq!(colored, 6);

	let cleaned: Vec<Solid> = cube.iter().map(|s| s.clean().expect("clean should succeed")).collect();

	// Every face in the cleaned shape must have a color.
	let mut colored_after = 0usize;
	for f in cleaned.iter().flat_map(|s| s.face_iter()) {
		assert!(colormap_contains(&cleaned, &f.tshape_id()), "face {:?} lost its color after clean", f.tshape_id());
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
	let mut a: Vec<Solid> = vec![Solid::cube(1.0, 1.0, 1.0)];
	color_box_faces(&mut a);

	// Box B: x ∈ [1,2], y ∈ [0,1], z ∈ [0,1]  (adjacent, sharing the x=1 face)
	let mut b: Vec<Solid> = vec![Solid::cube(1.0, 1.0, 1.0).translate(DVec3::new(1.0, 0.0, 0.0))];
	color_box_faces(&mut b);

	// Union produces a 2×1×1 slab whose side faces may be split at x=1.
	let unioned: Vec<Solid> = a.union(&b).expect("union should succeed");

	// clean() merges coplanar adjacent patches.
	let cleaned: Vec<Solid> = unioned.iter().map(|s| s.clean().expect("clean should succeed")).collect();

	// Every face in the cleaned shape must have a color.
	for f in cleaned.iter().flat_map(|s| s.face_iter()) {
		assert!(colormap_contains(&cleaned, &f.tshape_id()), "face {:?} lost its color after clean+merge", f.tshape_id());
	}
	// The 2×1×1 slab has 6 faces after clean.
	let face_count = cleaned.iter().flat_map(|s| s.face_iter()).count();
	assert_eq!(face_count, 6, "cleaned slab should have 6 faces, got {}", face_count);
	assert_eq!(colormap_len(&cleaned), 6, "all 6 faces should carry a color after clean");
}

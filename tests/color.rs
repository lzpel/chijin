#![cfg(feature = "color")]

use chijin::Shape;
use glam::DVec3;

#[test]
fn test_set_and_get_face_color() {
	let shape = Shape::box_from_corners(DVec3::new(0.0, 0.0, 0.0), DVec3::new(10.0, 10.0, 10.0));
	// 各面に異なる色を設定 (直方体は6面)
	let colors = [
		[255, 0, 0],   // Red
		[0, 255, 0],   // Green
		[0, 0, 255],   // Blue
		[255, 255, 0], // Yellow
		[255, 0, 255], // Magenta
		[0, 255, 255], // Cyan
	];
	let mut shape = shape;
	for (face, color) in shape.faces().zip(colors.iter()) {
		shape.set_face_color(&face, *color);
	}

	assert_eq!(shape.color_count(), 6);

	// 取得して一致確認
	for (face, expected) in shape.faces().zip(colors.iter()) {
		assert_eq!(shape.face_color(&face), Some(*expected));
	}
}

#[test]
fn test_boolean_preserves_colors() {
	let mut a = Shape::box_from_corners(DVec3::new(0.0, 0.0, 0.0), DVec3::new(10.0, 10.0, 10.0));
	a.set_all_faces_color([255, 0, 0]); // 赤

	let b = Shape::box_from_corners(DVec3::new(5.0, 5.0, 5.0), DVec3::new(15.0, 15.0, 15.0));

	let result = a.subtract(&b).unwrap();
	let result_shape: Shape = result.shape;

	// 元の面から引き継がれた面は赤のままのはず
	let mut red_count = 0;
	for face in result_shape.faces() {
		if let Some(color) = result_shape.face_color(&face) {
			assert_eq!(color, [255, 0, 0]);
			red_count += 1;
		}
	}
	assert!(
		red_count > 0,
		"At least some structural faces should be preserved and keep their color"
	);

	// new_faces (切断面) はデフォルトで色がついていない
	let new_faces = result.new_faces;
	for face in new_faces.faces() {
		assert_eq!(new_faces.face_color(&face), None);
	}
}

#[test]
fn test_clean_preserves_colors() {
	let mut a = Shape::box_from_corners(DVec3::new(0.0, 0.0, 0.0), DVec3::new(10.0, 10.0, 10.0));
	a.set_all_faces_color([0, 255, 0]); // 녹色

	let mut b = Shape::box_from_corners(
		DVec3::new(10.0, 0.0, 0.0), // aと隣接
		DVec3::new(20.0, 10.0, 10.0),
	);
	b.set_all_faces_color([0, 0, 255]); // 青色

	// 結合すると2つのボックスになり、間の面が余分に残る
	let union_result = a.union(&b).unwrap();
	let mut shape = union_result.shape;

	// unionしただけの状態での色確認 (緑か青のどちらかになっているはず)
	for face in shape.faces() {
		let color = shape.face_color(&face).unwrap();
		assert!(color == [0, 255, 0] || color == [0, 0, 255]);
	}

	// clean_shape_colored() を呼び出して、同一平面上の面をマージ
	let cleaned = shape.clean().unwrap();

	// クリーン後の面も色が保持されていなければならない
	// ※ 統合された面のどちらかの色が優先される
	let mut has_color = false;
	for face in cleaned.faces() {
		if let Some(color) = cleaned.face_color(&face) {
			assert!(color == [0, 255, 0] || color == [0, 0, 255]);
			has_color = true;
		}
	}
	assert!(has_color, "Colors should be preserved after clean()");
}

#[test]
fn test_translated_preserves_colors() {
	let mut a = Shape::box_from_corners(DVec3::new(0.0, 0.0, 0.0), DVec3::new(10.0, 10.0, 10.0));

	let colors = [
		[255, 0, 0],
		[0, 255, 0],
		[0, 0, 255],
		[255, 255, 0],
		[255, 0, 255],
		[0, 255, 255],
	];
	for (face, color) in a.faces().zip(colors.iter()) {
		a.set_face_color(&face, *color);
	}

	let b = a.translated(DVec3::new(10.0, 0.0, 0.0));

	// 色の対応がそのまま列挙順などで保持されるか
	for (face, expected) in b.faces().zip(colors.iter()) {
		assert_eq!(b.face_color(&face), Some(*expected));
	}
}

#[test]
fn test_step_color_roundtrip() {
	// Create a box with 6 distinctly-colored faces
	let colors: [[u8; 3]; 6] = [
		[255, 0, 0],   // Red
		[0, 255, 0],   // Green
		[0, 0, 255],   // Blue
		[255, 255, 0], // Yellow
		[255, 0, 255], // Magenta
		[0, 255, 255], // Cyan
	];

	let mut shape =
		Shape::box_from_corners(DVec3::new(0.0, 0.0, 0.0), DVec3::new(10.0, 10.0, 10.0));
	for (face, color) in shape.faces().zip(colors.iter()) {
		shape.set_face_color(&face, *color);
	}
	assert_eq!(shape.color_count(), 6);

	// Write to STEP bytes using XDE
	let mut step_data: Vec<u8> = Vec::new();
	shape
		.write_step_colored(&mut step_data)
		.expect("write_step_colored should succeed");
	assert!(!step_data.is_empty(), "STEP output should be non-empty");

	// Read back
	let mut cursor = std::io::Cursor::new(&step_data);
	let loaded =
		Shape::read_step_colored(&mut cursor).expect("read_step_colored should succeed");

	// Collect all colors present in the loaded shape
	let mut found: std::collections::HashSet<[u8; 3]> = Default::default();
	for face in loaded.faces() {
		if let Some(color) = loaded.face_color(&face) {
			found.insert(color);
		}
	}

	// All 6 distinct colors must survive the STEP round-trip.
	// (0 and 255 are exact in sRGB-linear conversion, so no rounding drift.)
	assert_eq!(
		found.len(),
		6,
		"All 6 face colors must be preserved after STEP round-trip, got: {:?}",
		found
	);
	for color in &colors {
		assert!(
			found.contains(color),
			"Color {:?} missing after round-trip",
			color
		);
	}
}

#[test]
fn test_stretch_preserves_colors() {
	// 1. 基本となる直方体を作成
	let mut base = Shape::box_from_corners(DVec3::new(0.0, 0.0, 0.0), DVec3::new(10.0, 10.0, 10.0));

	// 2. 各面に色を付ける (6面)
	let colors = [
		[255, 0, 0],
		[0, 255, 0],
		[0, 0, 255],
		[255, 255, 0],
		[255, 0, 255],
		[0, 255, 255],
	];
	for (face, color) in base.faces().zip(colors.iter()) {
		base.set_face_color(&face, *color);
	}

	// 3. stretchのシミュレーション:
	// (A) baseをX軸でカットするHalfSpaceツール
	let hs_keep = Shape::half_space(DVec3::new(5.0, 0.0, 0.0), DVec3::new(-1.0, 0.0, 0.0));

	// (B) cut実行
	let cut_result = base.subtract(&hs_keep).unwrap();
	let mut half_shape = cut_result.shape;

	// (C) 新規断面を取得（new_faces）し、stretch分(例: +5.0)だけ押し出す
	// 注: 現状のchijinにはprismがないため、boxで代用またはテスト用の簡易シミュレーション
	// ここでは単純に「色つきの半分」と「色つきの別の半分(translate済み)」をunionしてcleanする

	let mut second_half = base
		.subtract(&Shape::half_space(
			DVec3::new(5.0, 0.0, 0.0),
			DVec3::new(1.0, 0.0, 0.0),
		))
		.unwrap()
		.shape;
	second_half.set_global_translation(DVec3::new(5.0, 0.0, 0.0));

	// half_shape (x: 0..5), second_half (x: 10..15)
	// 間の（x: 5..10）を埋めるboxを作成して色を付ける
	let mut middle =
		Shape::box_from_corners(DVec3::new(5.0, 0.0, 0.0), DVec3::new(10.0, 10.0, 10.0));
	// side面の色を適当に塗る
	middle.set_all_faces_color([128, 128, 128]);

	// 全部まとめる
	let u1 = half_shape.union(&middle).unwrap().shape;
	let u2 = u1.union(&second_half).unwrap().shape;

	let final_shape = u2.clean().unwrap();

	// 最終的に色が保存されているかの確認。最低限、元からあった色は生き残るはず
	let mut color_count = 0;
	for face in final_shape.faces() {
		if final_shape.face_color(&face).is_some() {
			color_count += 1;
		}
	}
	// face_colorが取得できる面が存在すること
	assert!(color_count > 0);
}

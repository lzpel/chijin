use cadrum::{Compound, Solid};
use glam::DVec3;

fn dvec3(x: f64, y: f64, z: f64) -> DVec3 {
	DVec3::new(x, y, z)
}

/// 10×10×10 ボックス（体積 1000）
fn test_box() -> Solid {
	Solid::cube(10.0, 10.0, 10.0)
}

// ==================== translated ====================

#[test]
fn test_translated_preserves_volume() {
	let shape = test_box();
	let moved = shape.translate(dvec3(100.0, 200.0, -50.0));
	assert!((moved.volume() - 1000.0).abs() < 1e-6);
}

#[test]
fn test_translated_preserves_shell_count() {
	let shape = test_box();
	let moved = shape.translate(dvec3(5.0, 0.0, 0.0));
	assert_eq!(moved.shell_count(), 1);
}

#[test]
fn test_union_of_translated_overlapping_solids_has_single_volume() {
	// 異なる場所に同じ大きさの立方体を2つ作り、translatedで同じ場所に重ねてからunionする。
	// 結果のvolumeは1つ分（1000）になるはず。
	let a = [Solid::cube(10.0, 10.0, 10.0)];
	let b = [Solid::cube(10.0, 10.0, 10.0).translate(dvec3(100.0, 0.0, 0.0))];
	let b_moved: Vec<Solid> = b.clone().into_iter().map(|s| s.translate(dvec3(-100.0, 0.0, 0.0))).collect();

	// b と b_moved は実態が別であることを確認: a と b（移動前）を union するとvolumeは2つ分（2000）。
	let result_no_move: Vec<Solid> = a.union(&b).expect("union should succeed");
	let volume_no_move: f64 = result_no_move.iter().map(|s| s.volume()).sum();
	assert!((volume_no_move - 2000.0).abs() < 1e-3, "expected volume ~2000, got {volume_no_move}");

	// b_moved は a と完全に重なるので union すると1つ分（1000）。
	let result: Vec<Solid> = a.union(&b_moved).expect("union should succeed");
	let volume: f64 = result.iter().map(|s| s.volume()).sum();
	assert!((volume - 1000.0).abs() < 1e-3, "expected volume ~1000, got {volume}");

	// b_moved を作っても b は変化していないことを確認:
	// result（x=0付近, volume=1000）と b（x=100付近, volume=1000）を union すると2000になるはず。
	let result_with_b: Vec<Solid> = result.union(&b).expect("union should succeed");
	let volume_with_b: f64 = result_with_b.iter().map(|s| s.volume()).sum();
	assert!((volume_with_b - 2000.0).abs() < 1e-3, "expected volume ~2000, got {volume_with_b}");
}

// ==================== rotated ====================

#[test]
fn test_rotated_preserves_volume() {
	let shape = test_box();
	// Z 軸周りに 45° 回転
	let rotated = shape.rotate_z(std::f64::consts::FRAC_PI_4);
	assert!((rotated.volume() - 1000.0).abs() < 1e-3);
}

#[test]
fn test_rotated_full_turn_preserves_volume() {
	let shape = test_box();
	// 360° 回転（元に戻る）
	let rotated = shape.rotate_z(std::f64::consts::TAU);
	assert!((rotated.volume() - 1000.0).abs() < 1e-3);
}

#[test]
fn test_rotated_preserves_shell_count() {
	let shape = test_box();
	let rotated = shape.rotate_y(std::f64::consts::FRAC_PI_2);
	assert_eq!(rotated.shell_count(), 1);
}

// ==================== scale ====================

#[test]
fn test_scale_volume() {
	let shape = test_box();
	// 均一 2 倍スケール → 体積は 2³ = 8 倍
	let scaled = shape.scale(DVec3::ZERO, 2.0);
	assert!((scaled.volume() - 8000.0).abs() < 1e-3);
}

#[test]
fn test_scale_half_volume() {
	let shape = test_box();
	// 均一 0.5 倍スケール → 体積は (0.5)³ = 0.125 倍 = 125
	let scaled = shape.scale(DVec3::ZERO, 0.5);
	assert!((scaled.volume() - 125.0).abs() < 1e-3);
}

#[test]
fn test_scale_preserves_shell_count() {
	let shape = test_box();
	let scaled = shape.scale(DVec3::ZERO, 3.0);
	assert_eq!(scaled.shell_count(), 1);
}

// ==================== face id preservation ====================

#[test]
fn test_preserves_face_ids() {
	fn face_ids<'a>(s: impl IntoIterator<Item = &'a Solid>) -> Vec<u64> {
		s.into_iter().flat_map(|s| s.iter_face()).map(|f| f.tshape_id()).collect()
	}

	let shape = test_box();
	let solid_id = shape.tshape_id();
	let ids = face_ids([&shape]);
	let moved = shape.translate(dvec3(10.0, 0.0, 0.0));
	assert_eq!(solid_id, moved.tshape_id(), "translate should preserve solid tshape_id");
	assert_eq!(ids, face_ids([&moved]), "translate should preserve face IDs");

	let shape = test_box();
	let solid_id = shape.tshape_id();
	let ids = face_ids([&shape]);
	let rotated = shape.rotate_z(std::f64::consts::FRAC_PI_4);
	assert_eq!(solid_id, rotated.tshape_id(), "rotate should preserve solid tshape_id");
	assert_eq!(ids, face_ids([&rotated]), "rotate should preserve face IDs");
}

// ==================== is_tool_face / is_shape_face (B fully inside A) ====================

#[test]
fn test_new_faces_subtract_b_inside_a() {
	// small_box が big_box に完全に収まる → small の 6 面はすべて Modified されない
	// 旧実装（collect_generated_faces）では Modified() が空 → tool faces = 0
	// 新実装（from_b post_ids）では unchanged 面も from_b に入る → tool faces = 6
	let big = [Solid::cube(10.0, 10.0, 10.0)];
	let small = [Solid::cube(4.0, 4.0, 4.0).translate(dvec3(3.0, 3.0, 3.0))];
	let (solids, meta) = big.subtract_with_metadata(&small).unwrap();
	assert_eq!(solids.iter().flat_map(|s| s.iter_face()).filter(|f| cadrum::is_tool_face(&meta, f)).count(), 6, "subtract with B fully inside A: tool faces should be all 6 inner walls");
}

#[test]
fn test_new_faces_intersect_b_inside_a() {
	// intersect(big, small) の結果は small そのもの
	// small の 6 面はすべて unchanged → tool faces = 結果の全フェイス = 6
	let big = [Solid::cube(10.0, 10.0, 10.0)];
	let small = [Solid::cube(4.0, 4.0, 4.0).translate(dvec3(3.0, 3.0, 3.0))];
	let (solids, meta) = big.intersect_with_metadata(&small).unwrap();
	let tool_count = solids.iter().flat_map(|s| s.iter_face()).filter(|f| cadrum::is_tool_face(&meta, f)).count();
	assert_eq!(tool_count, 6, "intersect with B fully inside A: tool faces should equal all faces of result");
	assert_eq!(solids.iter().flat_map(|s| s.iter_face()).count(), tool_count, "intersect with B fully inside A: tool faces should cover all result faces");
}

// ==================== bounding_box ====================

#[test]
fn test_bounding_box() {
	// 単一 solid
	let [min, max] = Solid::cube(3.0, 4.0, 5.0).translate(dvec3(1.0, 2.0, 3.0)).bounding_box();
	assert!((min - dvec3(1.0, 2.0, 3.0)).length() < 1e-6);
	assert!((max - dvec3(4.0, 6.0, 8.0)).length() < 1e-6);

	// 複数 solid のマージ
	let solids = [Solid::cube(2.0, 2.0, 2.0), Solid::cube(2.0, 3.0, 4.0).translate(dvec3(5.0, 5.0, 5.0))];
	let bboxes: Vec<[DVec3; 2]> = solids.iter().map(|s| s.bounding_box()).collect();
	let min = bboxes.iter().map(|b| b[0]).reduce(|a, b| a.min(b)).unwrap();
	let max = bboxes.iter().map(|b| b[1]).reduce(|a, b| a.max(b)).unwrap();
	assert!((min - dvec3(0.0, 0.0, 0.0)).length() < 1e-6);
	assert!((max - dvec3(7.0, 8.0, 9.0)).length() < 1e-6);

	// translate 後に追従する
	let moved = test_box().translate(dvec3(10.0, 20.0, 30.0));
	let [min, max] = moved.bounding_box();
	assert!((min - dvec3(10.0, 20.0, 30.0)).length() < 1e-6);
	assert!((max - dvec3(20.0, 30.0, 40.0)).length() < 1e-6);

	// half-space は無限ソリッド — bbox がどう返るか確認
	let [min, max] = Solid::half_space(dvec3(0.0, 0.0, 0.0), dvec3(0.0, 0.0, 1.0)).bounding_box();
	println!("half_space bbox: min={min:?} max={max:?}");
}

// ==================== contains ====================

#[test]
fn test_contains() {
	let shape = test_box(); // 0..10 の箱
	assert!(shape.contains(dvec3(5.0, 5.0, 5.0))); // 中心
	assert!(shape.contains(dvec3(0.1, 0.1, 0.1))); // 内側寄り
	assert!(!shape.contains(dvec3(20.0, 5.0, 5.0))); // 外
	assert!(!shape.contains(dvec3(-0.1, 5.0, 5.0))); // 外寄り
}

use cadrum::{Shape, Solid};
use glam::DVec3;

fn dvec3(x: f64, y: f64, z: f64) -> DVec3 {
	DVec3::new(x, y, z)
}

/// 10×10×10 ボックス（体積 1000）
fn test_box() -> Vec<Solid> {
	vec![Solid::box_from_corners(dvec3(0.0, 0.0, 0.0), dvec3(10.0, 10.0, 10.0))]
}

// ==================== clone ====================

#[test]
fn test_clone_preserves_volume() {
	let original = test_box();
	let copy = original.clone();
	drop(original);
	assert!((copy.volume() - 1000.0).abs() < 1e-6);
}

#[test]
fn test_clone_is_independent() {
	// コピー後にオリジナルを boolean 操作しても copy は影響を受けない
	let original = test_box();
	let copy = original.clone();
	let other: Vec<Solid> = vec![Solid::box_from_corners(dvec3(5.0, 5.0, 5.0), dvec3(15.0, 15.0, 15.0))];
	let _: Vec<Solid> = cadrum::Boolean::union(&original, &other).unwrap().into();
	assert!((copy.volume() - 1000.0).abs() < 1e-6);
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
	let a = vec![Solid::box_from_corners(dvec3(0.0, 0.0, 0.0), dvec3(10.0, 10.0, 10.0))];
	let b = vec![Solid::box_from_corners(dvec3(100.0, 0.0, 0.0), dvec3(110.0, 10.0, 10.0))];
	let b_moved: Vec<Solid> = b.clone().translate(dvec3(-100.0, 0.0, 0.0));

	// b と b_moved は実態が別であることを確認: a と b（移動前）を union するとvolumeは2つ分（2000）。
	let result_no_move: Vec<Solid> = cadrum::Boolean::union(&a, &b)
		.expect("union should succeed")
		.into();
	let volume_no_move: f64 = result_no_move.iter().map(|s| s.volume()).sum();
	assert!((volume_no_move - 2000.0).abs() < 1e-3, "expected volume ~2000, got {volume_no_move}");

	// b_moved は a と完全に重なるので union すると1つ分（1000）。
	let result: Vec<Solid> = cadrum::Boolean::union(&a, &b_moved)
		.expect("union should succeed")
		.into();
	let volume: f64 = result.iter().map(|s| s.volume()).sum();
	assert!((volume - 1000.0).abs() < 1e-3, "expected volume ~1000, got {volume}");

	// b_moved を作っても b は変化していないことを確認:
	// result（x=0付近, volume=1000）と b（x=100付近, volume=1000）を union すると2000になるはず。
	let result_with_b: Vec<Solid> = cadrum::Boolean::union(&result, &b)
		.expect("union should succeed")
		.into();
	let volume_with_b: f64 = result_with_b.iter().map(|s| s.volume()).sum();
	assert!((volume_with_b - 2000.0).abs() < 1e-3, "expected volume ~2000, got {volume_with_b}");
}

// ==================== rotated ====================

#[test]
fn test_rotated_preserves_volume() {
	let shape = test_box();
	// Z 軸周りに 45° 回転
	let rotated = shape.rotate(DVec3::ZERO, DVec3::Z, std::f64::consts::FRAC_PI_4);
	assert!((rotated.volume() - 1000.0).abs() < 1e-3);
}

#[test]
fn test_rotated_full_turn_preserves_volume() {
	let shape = test_box();
	// 360° 回転（元に戻る）
	let rotated = shape.rotate(DVec3::ZERO, DVec3::Z, std::f64::consts::TAU);
	assert!((rotated.volume() - 1000.0).abs() < 1e-3);
}

#[test]
fn test_rotated_preserves_shell_count() {
	let shape = test_box();
	let rotated = shape.rotate(DVec3::ZERO, DVec3::Y, std::f64::consts::FRAC_PI_2);
	assert_eq!(rotated.shell_count(), 1);
}

// ==================== scaled ====================

#[test]
fn test_scaled_volume() {
	let shape = test_box();
	// 均一 2 倍スケール → 体積は 2³ = 8 倍
	let scaled = shape.scaled(DVec3::ZERO, 2.0);
	assert!((scaled.volume() - 8000.0).abs() < 1e-3);
}

#[test]
fn test_scaled_half_volume() {
	let shape = test_box();
	// 均一 0.5 倍スケール → 体積は (0.5)³ = 0.125 倍 = 125
	let scaled = shape.scaled(DVec3::ZERO, 0.5);
	assert!((scaled.volume() - 125.0).abs() < 1e-3);
}

#[test]
fn test_scaled_preserves_shell_count() {
	let shape = test_box();
	let scaled = shape.scaled(DVec3::ZERO, 3.0);
	assert_eq!(scaled.shell_count(), 1);
}

// ==================== face id preservation ====================

#[test]
fn test_preserves_face_ids() {
	use cadrum::TShapeId;
	fn face_ids(s: &Vec<Solid>) -> Vec<TShapeId> {
		s.faces().map(|f| f.tshape_id()).collect()
	}

	let shape = test_box();
	let solid_id = shape[0].tshape_id();
	let ids = face_ids(&shape);
	let moved = shape.translate(dvec3(10.0, 0.0, 0.0));
	assert_eq!(solid_id, moved[0].tshape_id(), "translate should preserve solid TShapeId");
	assert_eq!(ids, face_ids(&moved), "translate should preserve face IDs");

	let shape = test_box();
	let solid_id = shape[0].tshape_id();
	let ids = face_ids(&shape);
	let rotated = shape.rotate(DVec3::ZERO, DVec3::Z, std::f64::consts::FRAC_PI_4);
	assert_eq!(solid_id, rotated[0].tshape_id(), "rotate should preserve solid TShapeId");
	assert_eq!(ids, face_ids(&rotated), "rotate should preserve face IDs");
}

// ==================== mirrored / scaled independence ====================

#[test]
fn test_mirrored_octants_union_volume_is_eight() {
	// (1,1,1)→(2,2,2) のboxを全8方向に鏡像コピーして8辺体を作る。
	// 実態が独立していない（同一インスタンスなど）場合は重複で体積が8を下回る。
	let b = vec![Solid::box_from_corners(dvec3(1.0, 1.0, 1.0), dvec3(2.0, 2.0, 2.0))];
	let bx   = b.mirrored(DVec3::ZERO, DVec3::X);
	let by   = b.mirrored(DVec3::ZERO, DVec3::Y);
	let bz   = b.mirrored(DVec3::ZERO, DVec3::Z);
	let bxy  = bx.mirrored(DVec3::ZERO, DVec3::Y);
	let bxz  = bx.mirrored(DVec3::ZERO, DVec3::Z);
	let byz  = by.mirrored(DVec3::ZERO, DVec3::Z);
	let bxyz = bxy.mirrored(DVec3::ZERO, DVec3::Z);
	let octants = [b, bx, by, bz, bxy, bxz, byz, bxyz];
	let mut result = octants[0].clone();
	for other in &octants[1..] {
		result = cadrum::Boolean::union(&result, other).expect("union failed").into();
	}
	let volume: f64 = result.iter().map(|s| s.volume()).sum();
	assert!((volume - 8.0).abs() < 1e-3, "expected volume ~8, got {volume}");
}

#[test]
fn test_scaled_union_with_original_volume_is_nine() {
	// (1,1,1)→(2,2,2) のbox（体積1）と原点中心2倍スケール（→(2,2,2)→(4,4,4), 体積8）は
	// 角のみ接するので union 体積 = 1 + 8 = 9。
	// scaled が実態を変化させていた場合は体積がこれより小さくなる。
	let b = vec![Solid::box_from_corners(dvec3(1.0, 1.0, 1.0), dvec3(2.0, 2.0, 2.0))];
	let b_scaled = b.scaled(DVec3::ZERO, 2.0);
	let result: Vec<Solid> = cadrum::Boolean::union(&b, &b_scaled).expect("union failed").into();
	let volume: f64 = result.iter().map(|s| s.volume()).sum();
	assert!((volume - 9.0).abs() < 1e-3, "expected volume ~9 (1 + 8), got {volume}");
}

// ==================== Vec<Solid> operations (replaces into_solids / from_solids tests) ====================

#[test]
fn test_vec_solid_roundtrip() {
	// 2 つのボックスを Vec<Solid> として結合し、体積を確認
	let a = Solid::box_from_corners(dvec3(0.0, 0.0, 0.0), dvec3(10.0, 10.0, 10.0));
	let b = Solid::box_from_corners(dvec3(20.0, 0.0, 0.0), dvec3(30.0, 10.0, 10.0));
	let shape: Vec<Solid> = vec![a, b];
	let total_volume = shape.volume();

	assert_eq!(shape.len(), 2);

	// 各 solid の体積合計が元と一致
	let sum: f64 = shape.iter().map(|s| s.volume()).sum();
	assert!((sum - total_volume).abs() < 1e-6, "sum={sum}, expected={total_volume}");
}

#[test]
fn test_single_solid() {
	// 単一 solid → Vec に要素 1 個
	let shape = test_box();
	assert_eq!(shape.len(), 1);
	assert!((shape[0].volume() - 1000.0).abs() < 1e-6);
}

#[test]
fn test_empty_vec() {
	// 空 Vec
	let shape: Vec<Solid> = vec![];
	assert!(shape.is_empty());
}

// ==================== is_tool_face / is_shape_face (B fully inside A) ====================

#[test]
fn test_new_faces_subtract_b_inside_a() {
	// small_box が big_box に完全に収まる → small の 6 面はすべて Modified されない
	// 旧実装（collect_generated_faces）では Modified() が空 → tool faces = 0
	// 新実装（from_b post_ids）では unchanged 面も from_b に入る → tool faces = 6
	let big: Vec<Solid> = vec![Solid::box_from_corners(dvec3(0.0, 0.0, 0.0), dvec3(10.0, 10.0, 10.0))];
	let small: Vec<Solid> = vec![Solid::box_from_corners(dvec3(3.0, 3.0, 3.0), dvec3(7.0, 7.0, 7.0))];
	let result = cadrum::Boolean::subtract(&big, &small).unwrap();
	assert_eq!(result.solids.faces().filter(|f| result.is_tool_face(f)).count(), 6,
		"subtract with B fully inside A: tool faces should be all 6 inner walls");
}

#[test]
fn test_new_faces_intersect_b_inside_a() {
	// intersect(big, small) の結果は small そのもの
	// small の 6 面はすべて unchanged → tool faces = 結果の全フェイス = 6
	let big: Vec<Solid> = vec![Solid::box_from_corners(dvec3(0.0, 0.0, 0.0), dvec3(10.0, 10.0, 10.0))];
	let small: Vec<Solid> = vec![Solid::box_from_corners(dvec3(3.0, 3.0, 3.0), dvec3(7.0, 7.0, 7.0))];
	let result = cadrum::Boolean::intersect(&big, &small).unwrap();
	let tool_count = result.solids.faces().filter(|f| result.is_tool_face(f)).count();
	assert_eq!(tool_count, 6,
		"intersect with B fully inside A: tool faces should equal all faces of result");
	assert_eq!(result.solids.faces().count(), tool_count,
		"intersect with B fully inside A: tool faces should cover all result faces");
}

// ==================== bounding_box ====================

#[test]
fn test_bounding_box() {
	// 単一 solid
	let [min, max] = vec![Solid::box_from_corners(dvec3(1.0, 2.0, 3.0), dvec3(4.0, 6.0, 8.0))].bounding_box();
	assert!((min - dvec3(1.0, 2.0, 3.0)).length() < 1e-6);
	assert!((max - dvec3(4.0, 6.0, 8.0)).length() < 1e-6);

	// 複数 solid のマージ
	let [min, max] = vec![
		Solid::box_from_corners(dvec3(0.0, 0.0, 0.0), dvec3(2.0, 2.0, 2.0)),
		Solid::box_from_corners(dvec3(5.0, 5.0, 5.0), dvec3(7.0, 8.0, 9.0)),
	].bounding_box();
	assert!((min - dvec3(0.0, 0.0, 0.0)).length() < 1e-6);
	assert!((max - dvec3(7.0, 8.0, 9.0)).length() < 1e-6);

	// translate 後に追従する
	let [min, max] = test_box().translate(dvec3(10.0, 20.0, 30.0)).bounding_box();
	assert!((min - dvec3(10.0, 20.0, 30.0)).length() < 1e-6);
	assert!((max - dvec3(20.0, 30.0, 40.0)).length() < 1e-6);
}

// ==================== contains ====================

#[test]
fn test_contains() {
	let shape = test_box(); // 0..10 の箱
	assert!(Shape::contains(&shape, dvec3(5.0, 5.0, 5.0)));   // 中心
	assert!(Shape::contains(&shape, dvec3(0.1, 0.1, 0.1)));   // 内側寄り
	assert!(!Shape::contains(&shape, dvec3(20.0, 5.0, 5.0)));  // 外
	assert!(!Shape::contains(&shape, dvec3(-0.1, 5.0, 5.0)));  // 外寄り
}

use chijin::Shape;
use glam::DVec3;

fn dvec3(x: f64, y: f64, z: f64) -> DVec3 {
	DVec3::new(x, y, z)
}

/// 10×10×10 ボックス（体積 1000）
fn test_box() -> Shape {
	Shape::box_from_corners(dvec3(0.0, 0.0, 0.0), dvec3(10.0, 10.0, 10.0))
}

// ==================== deep_copy ====================

#[test]
fn test_deep_copy_preserves_volume() {
	let original = test_box();
	let copy = original.deep_copy();
	drop(original);
	assert!((copy.volume() - 1000.0).abs() < 1e-6);
}

#[test]
fn test_deep_copy_is_independent() {
	// コピー後にオリジナルを boolean 操作しても copy は影響を受けない
	let original = test_box();
	let copy = original.deep_copy();
	let other = Shape::box_from_corners(dvec3(5.0, 5.0, 5.0), dvec3(15.0, 15.0, 15.0));
	let _ = Shape::from(original.union(&other).unwrap());
	assert!((copy.volume() - 1000.0).abs() < 1e-6);
}

// ==================== translated ====================

#[test]
fn test_translated_preserves_volume() {
	let shape = test_box();
	let moved = shape.translated(dvec3(100.0, 200.0, -50.0));
	assert!((moved.volume() - 1000.0).abs() < 1e-6);
}

#[test]
fn test_translated_preserves_shell_count() {
	let shape = test_box();
	let moved = shape.translated(dvec3(5.0, 0.0, 0.0));
	assert_eq!(moved.shell_count(), 1);
}

// ==================== rotated ====================

#[test]
fn test_rotated_preserves_volume() {
	let shape = test_box();
	// Z 軸周りに 45° 回転
	let rotated = shape.rotated(DVec3::ZERO, DVec3::Z, std::f64::consts::FRAC_PI_4);
	assert!((rotated.volume() - 1000.0).abs() < 1e-3);
}

#[test]
fn test_rotated_full_turn_preserves_volume() {
	let shape = test_box();
	// 360° 回転（元に戻る）
	let rotated = shape.rotated(DVec3::ZERO, DVec3::Z, std::f64::consts::TAU);
	assert!((rotated.volume() - 1000.0).abs() < 1e-3);
}

#[test]
fn test_rotated_preserves_shell_count() {
	let shape = test_box();
	let rotated = shape.rotated(DVec3::ZERO, DVec3::Y, std::f64::consts::FRAC_PI_2);
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

// ==================== into_solids / from_solids ====================

#[test]
fn test_into_solids_roundtrip() {
	// 2 つのボックスを union して compound を作り、into_solids → from_solids のラウンドトリップ
	let a = Shape::box_from_corners(dvec3(0.0, 0.0, 0.0), dvec3(10.0, 10.0, 10.0));
	let b = Shape::box_from_corners(dvec3(20.0, 0.0, 0.0), dvec3(30.0, 10.0, 10.0));
	let compound = Shape::from_solids(vec![a, b]);
	let total_volume = compound.volume();

	// 分解
	let solids = compound.into_solids();
	assert_eq!(solids.len(), 2);

	// 各 solid の体積合計が元と一致
	let sum: f64 = solids.iter().map(|s| s.volume()).sum();
	assert!((sum - total_volume).abs() < 1e-6, "sum={sum}, expected={total_volume}");

	// 再合成
	let recompound = Shape::from_solids(solids);
	assert!((recompound.volume() - total_volume).abs() < 1e-6);
}

#[test]
fn test_into_solids_single() {
	// 単一 solid → into_solids で要素 1 個
	let shape = test_box();
	let solids = shape.into_solids();
	assert_eq!(solids.len(), 1);
	assert!((solids[0].volume() - 1000.0).abs() < 1e-6);
}

#[test]
fn test_into_solids_empty() {
	// 空 compound → into_solids で空 Vec
	let shape = Shape::empty();
	let solids = shape.into_solids();
	assert!(solids.is_empty());
}

// ==================== is_tool_face / is_shape_face (B fully inside A) ====================

#[test]
fn test_new_faces_subtract_b_inside_a() {
	// small_box が big_box に完全に収まる → small の 6 面はすべて Modified されない
	// 旧実装（collect_generated_faces）では Modified() が空 → tool faces = 0
	// 新実装（from_b post_ids）では unchanged 面も from_b に入る → tool faces = 6
	let big   = Shape::box_from_corners(dvec3(0.0, 0.0, 0.0), dvec3(10.0, 10.0, 10.0));
	let small = Shape::box_from_corners(dvec3(3.0, 3.0, 3.0), dvec3(7.0, 7.0, 7.0));
	let result = big.subtract(&small).unwrap();
	assert_eq!(result.shape.faces().filter(|f| result.is_tool_face(f)).count(), 6,
		"subtract with B fully inside A: tool faces should be all 6 inner walls");
}

#[test]
fn test_new_faces_intersect_b_inside_a() {
	// intersect(big, small) の結果は small そのもの
	// small の 6 面はすべて unchanged → tool faces = 結果の全フェイス = 6
	let big   = Shape::box_from_corners(dvec3(0.0, 0.0, 0.0), dvec3(10.0, 10.0, 10.0));
	let small = Shape::box_from_corners(dvec3(3.0, 3.0, 3.0), dvec3(7.0, 7.0, 7.0));
	let result = big.intersect(&small).unwrap();
	let tool_count = result.shape.faces().filter(|f| result.is_tool_face(f)).count();
	assert_eq!(tool_count, 6,
		"intersect with B fully inside A: tool faces should equal all faces of result");
	assert_eq!(result.shape.faces().count(), tool_count,
		"intersect with B fully inside A: tool faces should cover all result faces");
}

// ==================== contains ====================

#[test]
fn test_contains() {
	let shape = test_box(); // 0..10 の箱
	assert!(shape.contains(dvec3(5.0, 5.0, 5.0)));   // 中心
	assert!(shape.contains(dvec3(0.1, 0.1, 0.1)));   // 内側寄り
	assert!(!shape.contains(dvec3(20.0, 5.0, 5.0)));  // 外
	assert!(!shape.contains(dvec3(-0.1, 5.0, 5.0)));  // 外寄り
}

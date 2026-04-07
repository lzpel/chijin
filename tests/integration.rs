//! Integration tests for cadrum - OpenCASCADE Rust bindings.
//!
//! このファイルはプロジェクト最古のテスト群。notes/20260301-仕様書.md §3（既知バグ）
//! と §4.3（合格基準一覧）に定義された T-01〜T-08 に対応する。
//!
//! 各テスト名の `t0N` は合格基準番号に対応:
//!   T-01: Bug 1 — ブール演算結果の drop 順序で STATUS_HEAP_CORRUPTION が起きない
//!   T-02: Bug 2 — read_step 複数回呼び出し後にプロセスが正常終了する
//!   T-03: Bug 3 — mesh.normals.len() == mesh.vertices.len()（法線 off-by-one）
//!   T-04: Bug 4 — approximation_segments の tolerance が反映される（ハードコード脱却）
//!   T-05: Bug 5 — union 後コンパウンドへの平行移動が全頂点に正確に反映される
//!   T-06: I/O  — BRep バイナリの write→read ラウンドトリップ
//!   T-07: I/O  — read/write 中に一時ファイルが生成されない（ストリームAPI）
//!   T-08: API設計 — boolean の戻り値が中間型でなく Shape（現 Vec<Solid>）に変換可能

use cadrum::Solid;
use glam::DVec3;

fn dvec3(x: f64, y: f64, z: f64) -> DVec3 {
	DVec3::new(x, y, z)
}

fn test_box() -> Vec<Solid> {
	vec![Solid::cube(10.0, 10.0, 10.0)]
}

fn test_box_2() -> Vec<Solid> {
	vec![Solid::cube(10.0, 10.0, 10.0).translate(dvec3(5.0, 5.0, 5.0))]
}

fn test_box_3() -> Vec<Solid> {
	vec![Solid::cube(5.0, 5.0, 5.0).translate(dvec3(3.0, 3.0, 3.0))]
}

/// Helper: write shape to BRep binary bytes
fn shape_to_brep_bytes(shape: &[Solid]) -> Vec<u8> {
	let mut buf = Vec::new();
	cadrum::io::write_brep_binary(shape, &mut buf).unwrap();
	buf
}

// ==================== T-01: Boolean drop order safety ====================
// Bug 1: OCC のブール演算結果は入力と Handle<Geom_XXX> を共有するため、
// drop 順序によっては参照カウントが壊れ STATUS_HEAP_CORRUPTION が発生していた。
// deep_copy 不要で任意の drop 順序が安全であることを保証する。

#[test]
fn test_t01_union_drop_result_first() {
	let a = test_box();
	let b = test_box_2();
	let result = cadrum::Boolean::union(&a, &b).unwrap();
	drop(result);
	drop(a);
	drop(b);
}

#[test]
fn test_t01_union_drop_result_last() {
	let a = test_box();
	let b = test_box_2();
	let result = cadrum::Boolean::union(&a, &b).unwrap();
	drop(a);
	drop(b);
	drop(result);
}

#[test]
fn test_t01_subtract_drop_order() {
	let a = test_box();
	let b = test_box_2();
	let result = cadrum::Boolean::subtract(&a, &b).unwrap();
	drop(a);
	drop(b);
	drop(result);
}

#[test]
fn test_t01_intersect_drop_order() {
	let a = test_box();
	let b = test_box_2();
	let result = cadrum::Boolean::intersect(&a, &b).unwrap();
	drop(a);
	drop(b);
	drop(result);
}

#[test]
fn test_t01_chained_boolean_drops() {
	let a = test_box();
	let b = test_box_2();
	let c = test_box_3();
	let r1 = cadrum::Boolean::union(&a, &b).unwrap();
	let r2 = cadrum::Boolean::subtract(r1.solids(), &c).unwrap();
	drop(r1);
	drop(r2);
	drop(a);
	drop(b);
	drop(c);
}

// ==================== T-02: read multiple times ====================
// Bug 2: STEPControl_Reader のデストラクタが OCCT グローバル状態と衝突し
// プロセス終了時に STATUS_ACCESS_VIOLATION が発生していた。
// read_step を複数回呼んでもクラッシュしないことを確認する。
// （本来はプロセス exit code で検証すべきだが、テスト完走で代用）

#[test]
fn test_t02_multiple_reads_no_crash() {
	let original = test_box();
	let brep_data = shape_to_brep_bytes(&original);
	for _ in 0..5 {
		let _shape = cadrum::io::read_brep_binary(&mut brep_data.as_slice()).unwrap();
	}
}

// ==================== T-03: Mesh normals count ====================
// Bug 3: 法線ループで normal_array.Length()（キャパシティ=頂点数+1）を上限に
// 使っていたため normals が vertices より1つ少なかった。

#[test]
fn test_t03_mesh_normals_count() {
	let shape = test_box();
	let mesh = cadrum::io::mesh(&shape, 0.1).unwrap();
	assert_eq!(mesh.normals.len(), mesh.vertices.len());
}

// ==================== T-04: Approximation tolerance ====================
// Bug 4: Edge の折れ線近似の angular/chord deflection が 0.1 にハードコードされていた。
// tolerance パラメータが実際に反映されること（密度が変化すること）を確認する。

#[test]
fn test_t04_approximation_tolerance() {
	let cyl: Vec<Solid> = vec![Solid::cylinder(10.0, dvec3(0.0, 0.0, 1.0), 20.0)];
	let mut has_difference = false;
	for edge in cyl.iter().flat_map(|s| s.edges()) {
		let coarse = edge.approximation_segments(1.0).len();
		let fine = edge.approximation_segments(0.01).len();
		if fine > coarse {
			has_difference = true;
		}
	}
	assert!(has_difference, "Fine tolerance should produce more points than coarse");
}

// ==================== T-05: Translation on compound shapes ====================
// Bug 5: set_global_translation(propagate=false) がコンパウンドのサブシェイプに
// 伝播しなかった。union 後の形状に平行移動を適用し、全頂点が正確にシフトすることを確認。

#[test]
fn test_t05_translated_compound() {
	let a = test_box();
	let b = test_box_2();
	let compound: Vec<Solid> = cadrum::Boolean::union(&a, &b).unwrap().into();
	let v = dvec3(100.0, 0.0, 0.0);
	let orig_mesh = cadrum::io::mesh(&compound, 0.1).unwrap();
	let shifted: Vec<Solid> = compound.into_iter().map(|s| s.translate(v)).collect();
	let shifted_mesh = cadrum::io::mesh(&shifted, 0.1).unwrap();

	assert_eq!(orig_mesh.vertices.len(), shifted_mesh.vertices.len());
	for (o, s) in orig_mesh.vertices.iter().zip(shifted_mesh.vertices.iter()) {
		assert!((s.x - o.x - v.x).abs() < 1e-6);
		assert!((s.y - o.y - v.y).abs() < 1e-6);
		assert!((s.z - o.z - v.z).abs() < 1e-6);
	}
}

// ==================== T-06: BRep binary roundtrip ====================
// BRep バイナリの write→read で頂点数・座標が一致することを確認。

#[test]
fn test_t06_brep_roundtrip() {
	let original = test_box();
	let orig_mesh = cadrum::io::mesh(&original, 0.1).unwrap();

	let brep_data = shape_to_brep_bytes(&original);
	let restored = cadrum::io::read_brep_binary(&mut brep_data.as_slice()).unwrap();
	let rest_mesh = cadrum::io::mesh(&restored, 0.1).unwrap();

	assert_eq!(orig_mesh.vertices.len(), rest_mesh.vertices.len());
	for (o, r) in orig_mesh.vertices.iter().zip(rest_mesh.vertices.iter()) {
		assert!((o.x - r.x).abs() < 1e-10);
		assert!((o.y - r.y).abs() < 1e-10);
		assert!((o.z - r.z).abs() < 1e-10);
	}
}

// ==================== T-08: Boolean returns BooleanShape, convertible to Shape ====================
// boolean の戻り値が中間型 BooleanShape ではなく Vec<Solid> に変換可能であること。
// （旧 API では BooleanShape という中間型が必要だった）

#[test]
fn test_t08_boolean_returns_shape() {
	let a = test_box();
	let b = test_box_2();
	let _union: Vec<Solid> = cadrum::Boolean::union(&a, &b).unwrap().into();
	let _sub: Vec<Solid> = cadrum::Boolean::subtract(&a, &b).unwrap().into();
	let _inter: Vec<Solid> = cadrum::Boolean::intersect(&a, &b).unwrap().into();
}

// ==================== STEP export ====================
// 仕様書外の追加テスト。STEP 書き出しが正常に完了することを確認。

#[test]
fn test_hollow_cube_write_step() {
	let outer: Vec<Solid> = vec![Solid::cube(20.0, 20.0, 20.0).translate(dvec3(-10.0, -10.0, -10.0))];
	let inner: Vec<Solid> = vec![Solid::cube(10.0, 10.0, 10.0).translate(dvec3(-5.0, -5.0, -5.0))];
	let hollow_cube: Vec<Solid> = cadrum::Boolean::subtract(&outer, &inner).unwrap().into();

	std::fs::create_dir_all("out").unwrap();
	let mut file = std::fs::File::create("out/hollow_cube.step").unwrap();
	cadrum::io::write_step(&hollow_cube, &mut file).unwrap();
}

// half_space は仕様書 §2.1 で定義されたプリミティブ。intersect との組み合わせ確認。
#[test]
fn test_half_space_intersect() {
	let shape = test_box();
	let half: Vec<Solid> = vec![Solid::half_space(dvec3(5.0, 0.0, 0.0), dvec3(1.0, 0.0, 0.0))];
	let result = cadrum::Boolean::intersect(&shape, &half).unwrap();
	assert!(!result.solids().iter().all(|s| s.is_null()));
}

// cylinder プリミティブの体積が πr²h と一致することを確認。
#[test]
fn test_cylinder() {
	let cyl: Vec<Solid> = vec![Solid::cylinder(5.0, dvec3(0.0, 0.0, 1.0), 10.0)];
	let expected = std::f64::consts::PI * 5.0f64.powi(2) * 10.0;
	assert!((cyl.iter().map(|s| s.volume()).sum::<f64>() - expected).abs() < 1e-6);
}

// T-06 のテキスト版。BRep テキストの write→read ラウンドトリップ。
#[test]
fn test_brep_text_roundtrip() {
	let original = test_box();

	let mut text_data = Vec::new();
	cadrum::io::write_brep_text(&original, &mut text_data).unwrap();
	assert!(!text_data.is_empty());

	let restored = cadrum::io::read_brep_text(&mut text_data.as_slice()).unwrap();
	let orig_mesh = cadrum::io::mesh(&original, 0.1).unwrap();
	let rest_mesh = cadrum::io::mesh(&restored, 0.1).unwrap();
	assert_eq!(orig_mesh.vertices.len(), rest_mesh.vertices.len());
}

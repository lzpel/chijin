//! Integration tests for `Solid::bspline`.
//!
//! 2 field-period ステラレーター風トーラスを作って XZ/YZ 平面で 4 象限
//! に切り、180° 回転対称(s1 ≈ s3, s2 ≈ s4)を体積で検証する。
//! 周期方向の制御点変動が `sin(2φ)`/`cos(2φ)` で構成されているため
//! `phi → phi + π` のシフトが離散グリッドを完全に保存する → 近似誤差を
//! 導入しないので、対称性は boolean op の数値ノイズ分しか揺れない想定。

use cadrum::Solid;
use glam::{DQuat, DVec3};
use std::f64::consts::TAU;

/// solid を out/ 以下に SVG, STL, STEP で書き出す。
fn write_outputs(solids: &[Solid], name: &str) {
	std::fs::create_dir_all("out").unwrap();
	let mut f = std::fs::File::create(format!("out/{name}.step")).unwrap();
	cadrum::write_step(solids, &mut f).expect("step write");
	let mut f = std::fs::File::create(format!("out/{name}.stl")).unwrap();
	cadrum::mesh(solids, 0.1).and_then(|m| m.write_stl(&mut f)).expect("stl write");
	let mut f = std::fs::File::create(format!("out/{name}.svg")).unwrap();
	cadrum::mesh(solids, 0.5).and_then(|m| m.write_svg(DVec3::new(1.0, 1.0, 2.0), true, false, &mut f)).expect("svg write");
}

/// XZ 平面(法線 Y)と YZ 平面(法線 X)で 4 象限に分割し、180° 回転対称
/// (s1 ≈ s3, s2 ≈ s4)を体積で検証する。`tol` は相対誤差閾値。
fn assert_quadrant_point_symmetry(solid: &Solid, tol: f64) {
	let total = solid.volume();
	assert!(total > 0.0, "volume should be positive, got {}", total);

	// 各 half_space は法線の向きに solid が満ちる。
	let plus_x = Solid::half_space(DVec3::ZERO, DVec3::X);
	let minus_x = Solid::half_space(DVec3::ZERO, -DVec3::X);
	let plus_y = Solid::half_space(DVec3::ZERO, DVec3::Y);
	let minus_y = Solid::half_space(DVec3::ZERO, -DVec3::Y);

	let quadrant = |hs1: &Solid, hs2: &Solid| -> f64 {
		let (ab, _) = Solid::boolean_intersect(std::slice::from_ref(solid), std::slice::from_ref(hs1)).expect("intersect hs1");
		let (q, _) = Solid::boolean_intersect(&ab, std::slice::from_ref(hs2)).expect("intersect hs2");
		q.iter().map(|s| s.volume()).sum::<f64>()
	};

	let s1 = quadrant(&plus_x, &plus_y); // +X, +Y
	let s2 = quadrant(&minus_x, &plus_y); // -X, +Y
	let s3 = quadrant(&minus_x, &minus_y); // -X, -Y
	let s4 = quadrant(&plus_x, &minus_y); // +X, -Y

	let sum = s1 + s2 + s3 + s4;
	println!("total={:.6}, s1={:.6}, s2={:.6}, s3={:.6}, s4={:.6}, sum={:.6}", total, s1, s2, s3, s4, sum);

	// 180° 点対称: s1 ≈ s3, s2 ≈ s4
	let avg13 = (s1 + s3) / 2.0;
	let avg24 = (s2 + s4) / 2.0;
	let err13 = (s1 - s3).abs() / avg13;
	let err24 = (s2 - s4).abs() / avg24;
	println!("point symmetry: s1-s3 rel_err={:.6}, s2-s4 rel_err={:.6}", err13, err24);

	assert!(err13 < tol, "s1={:.4} vs s3={:.4} (rel err {:.4} >= {:.4})", s1, s3, err13, tol);
	assert!(err24 < tol, "s2={:.4} vs s4={:.4} (rel err {:.4} >= {:.4})", s2, s4, err24, tol);
}

// ==================== (1) 2-period stellarator-like torus ====================

#[test]
fn test_bspline_01_two_period_torus_point_symmetry() {
	const M: usize = 48; // toroidal (U) — 180° 対称のため偶数
	const N: usize = 24; // poloidal (V) — 任意
	const RING_R: f64 = 6.0;

	// 2 field-period ステラレーター風トーラス。以下すべて phi → phi+π で
	// 不変(または 2π の倍数分だけずれる)ため 180° 回転対称を保つ:
	//   a(phi)      = 1.8 + 0.6 * sin(2φ)       radial 半軸
	//   b(phi)      = 1.0 + 0.4 * cos(2φ)       Z 半軸
	//   psi(phi)    = 2 * phi                   cross-section ひねり(1周で2回転)
	//   z_shift(phi)= 1.0 * sin(2φ)             上下方向のうねり
	// psi(phi+π) = 2phi+2π ≡ 2phi (mod 2π) → 楕円の向きは同じ
	// z_shift(phi+π) = sin(2phi+2π) = sin(2phi) → 同じ高さ
	// a/b も同様に同じ値 → 形状は phi+π でも同一 → Z 軸まわり 180° 対称。
	let grid: [[DVec3; N]; M] = std::array::from_fn(|i| {
		std::array::from_fn(|j| {
			let phi = TAU * (i as f64) / (M as f64);
			let theta = TAU * (j as f64) / (N as f64);
			let two_phi = 2.0 * phi;
			let a = 1.8 + 0.6 * two_phi.sin();
			let b = 1.0 + 0.4 * two_phi.cos();
			let psi = two_phi; // ひねり 2 回転 per loop
			let z_shift = 1.0 * two_phi.sin();
			// 1. 局所断面(まだひねる前、(X,Z) 平面の楕円)
			let local_raw = DVec3::X * (a * theta.cos()) + DVec3::Z * (b * theta.sin());
			// 2. 局所 Y 軸(大径接線方向)まわりに psi 回転 — これが断面のひねり
			let local_twisted = DQuat::from_axis_angle(DVec3::Y, psi) * local_raw;
			// 3. 局所フレームで上下に揺らす
			let local_shifted = local_twisted + DVec3::Z * z_shift;
			// 4. 大径方向に RING_R だけ外へ
			let translated = local_shifted + DVec3::X * RING_R;
			// 5. 全体として Z 軸まわりに phi 回転
			DQuat::from_axis_angle(DVec3::Z, phi) * translated
		})
	});

	let plasma = Solid::bspline(grid, true).expect("2-period bspline torus should succeed");
	assert_eq!(plasma.shell_count(), 1);
	assert!(plasma.volume() > 0.0);

	assert_quadrant_point_symmetry(&plasma, 0.01);

	write_outputs(&[plasma, Solid::bspline(grid, false).unwrap().translate(DVec3::Z * -10.0)], "test_bspline_01_two_period_torus");
}

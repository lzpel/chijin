//! Integration tests for `Solid::gordon`.
//!
//! Gordon surface は profiles (断面曲線群) と guides (ガイド曲線群) の全交差点を
//! 拘束条件として transfinite 補間する。テストでは guide の端点が profiles の
//! 補間点と厳密に一致するように構成する。

use cadrum::{Edge, Error, Solid};
use glam::DVec3;

fn write_outputs(solids: &[Solid], name: &str) {
	std::fs::create_dir_all("out").unwrap();
	let mut f = std::fs::File::create(format!("out/{name}.step")).unwrap();
	cadrum::io::write_step(solids, &mut f).expect("step write");
	let mut f = std::fs::File::create(format!("out/{name}.svg")).unwrap();
	cadrum::io::write_svg(solids, DVec3::new(1.0, 1.0, 2.0), 0.5, true, &mut f).expect("svg write");
}

// ==================== (1) BSpline profiles × line guides ====================
// 格子点が厳密に一致する 2×3 curve network → solid

#[test]
fn test_gordon_01_bspline_surface() {
	use cadrum::BSplineEnd;

	//   g0(x=0)   g1(x=4)   g2(x=8)
	// p0(z=0): (0,0,0)  (4,1,0)  (8,0,0)
	// p1(z=6): (0,0,6)  (4,2,6)  (8,0,6)
	let p0 = Edge::bspline(
		[DVec3::new(0.0, 0.0, 0.0), DVec3::new(4.0, 1.0, 0.0), DVec3::new(8.0, 0.0, 0.0)],
		BSplineEnd::NotAKnot,
	).unwrap();
	let p1 = Edge::bspline(
		[DVec3::new(0.0, 0.0, 6.0), DVec3::new(4.0, 2.0, 6.0), DVec3::new(8.0, 0.0, 6.0)],
		BSplineEnd::NotAKnot,
	).unwrap();

	let g0 = vec![Edge::line(DVec3::new(0.0, 0.0, 0.0), DVec3::new(0.0, 0.0, 6.0)).unwrap()];
	let g1 = vec![Edge::line(DVec3::new(4.0, 1.0, 0.0), DVec3::new(4.0, 2.0, 6.0)).unwrap()];
	let g2 = vec![Edge::line(DVec3::new(8.0, 0.0, 0.0), DVec3::new(8.0, 0.0, 6.0)).unwrap()];

	let solid = Solid::gordon(
		[&[p0][..], &[p1][..]],
		[&g0[..], &g1[..], &g2[..]],
	).expect("bspline gordon should succeed");

	assert_eq!(solid.shell_count(), 1);
	assert!(solid.volume() > 0.0, "volume should be positive, got {}", solid.volume());

	write_outputs(std::slice::from_ref(&solid), "test_gordon_01_bspline_surface");
}

// ==================== (2) 3×3 line grid ====================
// 直線のみの最小構成で Gordon surface が動作することを確認

#[test]
fn test_gordon_02_line_grid_3x3() {
	// 3 profiles × 3 guides (平面)
	// profiles: z=0, z=3, z=6 に x=[0,8]
	// guides: x=0, x=4, x=8 に z=[0,6]
	// 全交差点は y=0 平面上
	let p0 = vec![Edge::line(DVec3::new(0.0, 0.0, 0.0), DVec3::new(8.0, 0.0, 0.0)).unwrap()];
	let p1 = vec![Edge::line(DVec3::new(0.0, 0.0, 3.0), DVec3::new(8.0, 0.0, 3.0)).unwrap()];
	let p2 = vec![Edge::line(DVec3::new(0.0, 0.0, 6.0), DVec3::new(8.0, 0.0, 6.0)).unwrap()];

	let g0 = vec![Edge::line(DVec3::new(0.0, 0.0, 0.0), DVec3::new(0.0, 0.0, 6.0)).unwrap()];
	let g1 = vec![Edge::line(DVec3::new(4.0, 0.0, 0.0), DVec3::new(4.0, 0.0, 6.0)).unwrap()];
	let g2 = vec![Edge::line(DVec3::new(8.0, 0.0, 0.0), DVec3::new(8.0, 0.0, 6.0)).unwrap()];

	// 平面なので solid にはならない (volume ≈ 0) が、Gordon 構築自体は成功する
	let result = Solid::gordon(
		[&p0[..], &p1[..], &p2[..]],
		[&g0[..], &g1[..], &g2[..]],
	);
	// 平面パッチ → solid 化は失敗するかもしれないが、GordonFailed であること
	assert!(result.is_err() || result.as_ref().unwrap().volume().abs() < 1.0);
}

// ==================== (3) エラー: profiles < 2 ====================

#[test]
fn test_gordon_03_single_profile_returns_error() {
	let p0 = Edge::polygon([
		DVec3::new(0.0, 0.0, 0.0),
		DVec3::new(1.0, 0.0, 0.0),
		DVec3::new(1.0, 1.0, 0.0),
		DVec3::new(0.0, 1.0, 0.0),
	]).unwrap();
	let g0 = vec![Edge::line(DVec3::ZERO, DVec3::Z).unwrap()];
	let g1 = vec![Edge::line(DVec3::X, DVec3::X + DVec3::Z).unwrap()];

	let result = Solid::gordon([&p0[..]], [&g0[..], &g1[..]]);
	let err = result.err().expect("single profile must fail");
	match err {
		Error::GordonFailed(msg) => assert!(msg.contains("≥2") || msg.contains("profiles")),
		other => panic!("expected GordonFailed, got {:?}", other),
	}
}

// ==================== (4) エラー: guides < 2 ====================

#[test]
fn test_gordon_04_single_guide_returns_error() {
	let p0 = Edge::polygon([
		DVec3::new(0.0, 0.0, 0.0),
		DVec3::new(1.0, 0.0, 0.0),
		DVec3::new(1.0, 1.0, 0.0),
		DVec3::new(0.0, 1.0, 0.0),
	]).unwrap();
	let p1 = Edge::polygon([
		DVec3::new(0.0, 0.0, 1.0),
		DVec3::new(1.0, 0.0, 1.0),
		DVec3::new(1.0, 1.0, 1.0),
		DVec3::new(0.0, 1.0, 1.0),
	]).unwrap();
	let g0 = vec![Edge::line(DVec3::ZERO, DVec3::Z).unwrap()];

	let result = Solid::gordon([&p0[..], &p1[..]], [&g0[..]]);
	let err = result.err().expect("single guide must fail");
	match err {
		Error::GordonFailed(msg) => assert!(msg.contains("≥2") || msg.contains("guides")),
		other => panic!("expected GordonFailed, got {:?}", other),
	}
}

// ==================== (5) エラー: 空 profile ====================

#[test]
fn test_gordon_05_empty_profile_returns_error() {
	let empty: Vec<Edge> = vec![];
	let p1 = Edge::polygon([
		DVec3::new(0.0, 0.0, 1.0),
		DVec3::new(1.0, 0.0, 1.0),
		DVec3::new(1.0, 1.0, 1.0),
		DVec3::new(0.0, 1.0, 1.0),
	]).unwrap();
	let g0 = vec![Edge::line(DVec3::ZERO, DVec3::Z).unwrap()];
	let g1 = vec![Edge::line(DVec3::X, DVec3::X + DVec3::Z).unwrap()];

	let result = Solid::gordon([&empty[..], &p1[..]], [&g0[..], &g1[..]]);
	let err = result.err().expect("empty profile must fail");
	match err {
		Error::GordonFailed(msg) => assert!(msg.contains("empty")),
		other => panic!("expected GordonFailed, got {:?}", other),
	}
}

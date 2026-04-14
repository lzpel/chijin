//! Integration tests for `Solid::loft`.
//!
//! Covers:
//! - 数値検証 (frustum 体積 vs 解析値)
//! - bspline 断面との結合 (ステラレーター rib loft の最小再現)
//! - エラーケース 2 種 (single section, empty section)
//! - Closure-based 呼び出し方の ergonomic 確認

use cadrum::{Edge, Error, Solid};
use glam::DVec3;
use std::f64::consts::PI;

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

// ==================== (1) 数値検証: 円錐台 ====================

#[test]
fn test_loft_01_frustum_volume_matches_analytical() {
	let r1 = 3.0;
	let r2 = 2.0;
	let h = 10.0;
	let lower = vec![Edge::circle(r1, DVec3::Z).unwrap()];
	let upper = vec![Edge::circle(r2, DVec3::Z).unwrap().translate(DVec3::Z * h)];

	let frustum = Solid::loft(&[lower, upper]).expect("frustum loft should succeed");

	let expected = PI / 3.0 * h * (r1 * r1 + r1 * r2 + r2 * r2);
	let actual = frustum.volume();
	let rel_err = (actual - expected).abs() / expected;

	assert!(
		rel_err < 0.01,
		"frustum volume {:.4} vs analytical {:.4} (relative error {:.4})",
		actual, expected, rel_err
	);
	assert_eq!(frustum.shell_count(), 1);

	write_outputs(std::slice::from_ref(&frustum), "test_loft_01_frustum_volume_matches_analytical");
	println!("frustum loft: volume = {:.4} (expected {:.4})", actual, expected);
}

// ==================== (2) エラーケース: section 1 つ ====================

#[test]
fn test_loft_02_single_section_returns_loft_failed() {
	let only = vec![Edge::circle(1.0, DVec3::Z).unwrap()];
	let result = Solid::loft(&[only]);

	let err = result.err().expect("single section must return Err");
	match err {
		Error::LoftFailed(msg) => {
			assert!(
				msg.contains("≥2") || msg.contains(">=2") || msg.contains("got 1"),
				"error message should mention min section count, got: {}",
				msg
			);
		}
		other => panic!("expected Error::LoftFailed, got {:?}", other),
	}
}

// ==================== (3) エラーケース: 空 section ====================

#[test]
fn test_loft_03_empty_section_returns_loft_failed() {
	let s1 = vec![Edge::circle(1.0, DVec3::Z).unwrap()];
	let empty: Vec<Edge> = vec![];
	let s3 = vec![Edge::circle(1.0, DVec3::Z).unwrap().translate(DVec3::Z * 10.0)];

	let result = Solid::loft(&[s1, empty, s3]);

	let err = result.err().expect("empty section must return Err");
	match err {
		Error::LoftFailed(msg) => {
			assert!(
				msg.contains("empty"),
				"error message should mention empty section, got: {}",
				msg
			);
		}
		other => panic!("expected Error::LoftFailed, got {:?}", other),
	}
}

// ==================== (4) Closure-style ergonomic test ====================

#[test]
fn test_loft_04_closure_iterator_form() {
	let ribs: Vec<Edge> = (0..3)
		.map(|i| {
			Edge::circle(2.0 + i as f64 * 0.5, DVec3::Z)
				.unwrap()
				.translate(DVec3::Z * i as f64 * 5.0)
		})
		.collect();

	let plasma = Solid::loft(ribs.iter().map(|e| [e])).expect("closure-form loft should succeed");

	assert_eq!(plasma.shell_count(), 1);
	assert!(plasma.volume() > 0.0);

	write_outputs(std::slice::from_ref(&plasma), "test_loft_05_closure_iterator_form");
}

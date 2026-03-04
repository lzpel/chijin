// 目的: ランダムに中心点を変えながら引き延ばし処理を多数実行し、
// 実行時エラー(C++例外・パニック)が生じるパラメータの組み合わせを探索して
// out/stretch_box_random_survey.csv に書き出す。テスト自体はエラーが生じても継続し、
// 最終的に全試行の成功・失敗をCSVで記録することを目的とする。

use chijin::{Error, Shape};
use glam::DVec3;
use std::panic::{self, AssertUnwindSafe};
use std::path::Path;

// ── ユーティリティ ────────────────────────────────────────────────

/// 切断面フェイスの Compound を delta 方向に押し出してフィラーを作ります。
/// BooleanShape::new_faces から直接フェイスを受け取るため、
/// heuristic による法線・重心フィルタは不要です。
fn extrude_faces(cut_faces: &Shape, delta: DVec3) -> Result<Shape, Error> {
	let mut filler: Option<Shape> = None;
	for face in cut_faces.faces() {
		let extruded = Shape::from(face.extrude(delta)?);
		filler = Some(match filler {
			None => extruded,
			Some(f) => Shape::from(f.union(&extruded)?),
		});
	}
	Ok(filler.unwrap_or_else(Shape::empty))
}

/// 指定された座標とベクトルで形状を分割し、片方を平行移動させた後、隙間を押し出し形状で埋めることで引き伸ばしを行います。
/// intersect の BooleanShape::new_faces から切断面を直接取得するため、
/// 法線・重心による heuristic フィルタを使いません。
fn stretch_vector(shape: &Shape, origin: DVec3, delta: DVec3) -> Result<Shape, Error> {
	// Negate so the solid fills the -delta side; intersect then yields part_neg.
	let half = Shape::half_space(origin, -delta.normalize());

	let intersect_result = shape.intersect(&half)?;
	let part_neg = intersect_result.shape;
	let cut_faces = intersect_result.new_faces;
	let part_pos = Shape::from(shape.subtract(&half)?).translated(delta);

	let filler = extrude_faces(&cut_faces, delta)?;
	let combined = Shape::from(part_neg.union(&filler)?);
	combined.union(&part_pos).map(Shape::from)
}

/// 形状をX, Y, Zの各軸方向に順番に引き伸ばします。
fn stretch(shape: &Shape, cx: f64, cy: f64, cz: f64, dx: f64, dy: f64, dz: f64) -> Result<Shape, Error> {
	let eps = 1e-10;
	let origin = DVec3::new(cx, cy, cz);

	let x;
	let after_x: &Shape = if dx > eps {
		x = stretch_vector(shape, origin, DVec3::new(dx, 0.0, 0.0))?;
		&x
	} else {
		shape
	};

	let y;
	let after_y: &Shape = if dy > eps {
		y = stretch_vector(after_x, origin, DVec3::new(0.0, dy, 0.0))?;
		&y
	} else {
		after_x
	};

	let z;
	let after_z: &Shape = if dz > eps {
		z = stretch_vector(after_y, origin, DVec3::new(0.0, 0.0, dz))?;
		&z
	} else {
		after_y
	};

	after_z.clean()
}

// ── 今回の要件 ──────────────────────────────────────────────────

/// テスト用のベース形状として、外部のSTEPファイルを読み込みます。
pub fn lambda360box() -> Shape {
	let mut file = std::fs::File::open(
		"steps/LAMBDA360-BOX-d6cb2eb2d6e0d802095ea1eda691cf9a3e9bf3394301a0d148f53e55f0f97951.step",
	)
	.expect("Failed to open step file");
	Shape::read_step(&mut file).expect("Failed to read step file")
}

/// 形状の引き伸ばし処理をパニックキャッチ付きで安全に実行し、結果を返します。
pub fn stretch_ok(
	shape: &Shape,
	cx: f64,
	cy: f64,
	cz: f64,
	dx: f64,
	dy: f64,
	dz: f64,
) -> Result<Shape, String> {
	// C++ exceptions are caught at the FFI boundary and returned as Err.
	// panic::catch_unwind is kept as a safety net for unexpected Rust panics.
	let result = panic::catch_unwind(AssertUnwindSafe(|| stretch(shape, cx, cy, cz, dx, dy, dz)));

	match result {
		Ok(Ok(s)) => Ok(s),
		Ok(Err(e)) => Err(e.to_string()),
		Err(payload) => {
			let msg = if let Some(s) = payload.downcast_ref::<&str>() {
				(*s).to_string()
			} else if let Some(s) = payload.downcast_ref::<String>() {
				s.clone()
			} else {
				"Unknown panic in shape operations".to_string()
			};
			Err(msg)
		}
	}
}

/// 線形合同法(LCG)によるシンプルな疑似乱数生成器です。
struct Lcg {
	state: u32,
}
impl Lcg {
	/// 指定されたシードで乱数生成器を初期化します。
	fn new(seed: u32) -> Self {
		Self { state: seed }
	}
	/// 0.0以上1.0未満の乱数を生成します。
	fn next_f64(&mut self) -> f64 {
		self.state = self.state.wrapping_mul(1664525).wrapping_add(1013904223);
		(self.state as f64) / (u32::MAX as f64)
	}
	/// 指定された範囲[min, max)の乱数を生成します。
	fn gen_range(&mut self, min: f64, max: f64) -> f64 {
		min + self.next_f64() * (max - min)
	}
}

#[test]
#[ignore = "3000 試行で約 8 分かかるため通常の cargo test からは除外。実行: cargo test stretch_box_random_survey -- --ignored"]
/// ランダムなパラメータで引き伸ばし処理を多数実行し、成功・失敗の結果を
/// out/stretch_box_random_survey.csv に出力します。
fn stretch_box_random_survey() {
	use std::io::Write;

	let out_dir = Path::new("out");
	if !out_dir.exists() {
		std::fs::create_dir_all(out_dir).unwrap();
	}

	let mut file = std::fs::File::create("out/stretch_box_random_survey.csv").unwrap();
	writeln!(file, "cx,cy,cz,dx,dy,dz,success,error_msg").unwrap();

	let base_shape = lambda360box();
	let mut rng = Lcg::new(42);

	let mut success_count = 0;
	let total_attempts = 1000;

	for _i in 0..total_attempts {
		let cx = rng.gen_range(-15.0, 15.0);
		let cy = rng.gen_range(-15.0, 15.0);
		let cz = rng.gen_range(-15.0, 15.0);

		for (dx, dy, dz) in [(1.0, 0.0, 0.0), (0.0, 1.0, 0.0), (0.0, 0.0, 1.0)] {
			match stretch_ok(&base_shape, cx, cy, cz, dx, dy, dz) {
				Ok(_) => {
					success_count += 1;
					writeln!(file, "{},{},{},{},{},{},1,", cx, cy, cz, dx, dy, dz).unwrap();
				}
				Err(e) => {
					writeln!(file, "{},{},{},{},{},{},0,{}", cx, cy, cz, dx, dy, dz, e).unwrap();
				}
			}
		}
	}

	println!(
		"Out of {} total tries, {} succeeded.",
		total_attempts * 3,
		success_count
	);
}

// ── STEP 出力ユーティリティ ──────────────────────────────────────

/// 形状を `out/<name>.step` に書き出し、頂点数・三角形数を標準出力に表示します。
/// `out/` ディレクトリが存在しない場合は自動作成します。
fn write_step(shape: &Shape, name: &str) {
	std::fs::create_dir_all("out").unwrap();
	let path = format!("out/{name}.step");
	let mut file = std::fs::File::create(&path).unwrap();
	shape.write_step(&mut file).expect("STEP write failed");
	let mesh = shape.mesh_with_tolerance(0.1).expect("meshing failed");
	assert!(!mesh.vertices.is_empty(), "result shape has no vertices");
	println!(
		"{name}: {} vertices, {} triangles → {path}",
		mesh.vertices.len(),
		mesh.indices.len() / 3,
	);
}

// ── 診断テスト ───────────────────────────────────────────────────

/// new_faces (Generated() 結果) の中身を診断する。
/// half_space と大きな有界ボックスの2種類のツールで intersect し、
/// new_faces に何フェイスあるかを比較することで、Generated() が
/// 非有界フェイスに対して機能しないかを確認する。
#[test]
fn diagnose_new_faces() {
	let shape = lambda360box();
	println!("input: face_count={}, shell_count={}", shape.faces().count(), shape.shell_count());

	let origin = glam::DVec3::new(1.0, 0.0, 1.0);
	let delta = glam::DVec3::new(1.0, 0.0, 0.0);

	// --- tool A: half_space (非有界フェイス) ---
	// Negate so the solid fills the -delta side; intersect then yields part_neg.
	let half = Shape::half_space(origin, -delta.normalize());
	println!("\n[tool=half_space]");
	println!("  half_space: face_count={}, shell_count={}", half.faces().count(), half.shell_count());
	let r_half = shape.intersect(&half).expect("intersect(half_space) failed");
	println!(
		"  intersect result: shape face_count={}, shell_count={}",
		r_half.shape.faces().count(),
		r_half.shape.shell_count()
	);
	println!("  intersect result: new_faces face_count={}", r_half.new_faces.faces().count());

	// --- tool B: 大きな有界ボックス (origin.x=1.0 が切断面になるように配置) ---
	// x が [-1000, 1.0] の範囲の巨大ボックスで x=1.0 面が切断面となる
	let big_box = Shape::box_from_corners(
		glam::DVec3::new(-1000.0, -1000.0, -1000.0),
		glam::DVec3::new(1.0, 1000.0, 1000.0),
	);
	println!("\n[tool=big_box (bounded, x in [-1000,1.0])]");
	println!("  big_box: face_count={}, shell_count={}", big_box.faces().count(), big_box.shell_count());
	let r_box = shape.intersect(&big_box).expect("intersect(big_box) failed");
	println!(
		"  intersect result: shape face_count={}, shell_count={}",
		r_box.shape.faces().count(),
		r_box.shape.shell_count()
	);
	println!("  intersect result: new_faces face_count={}", r_box.new_faces.faces().count());
	println!("  new_faces details:");
	for (i, face) in r_box.new_faces.faces().enumerate() {
		let n = face.normal_at_center();
		let c = face.center_of_mass();
		println!("    face[{i}]: normal=({:.3},{:.3},{:.3}) center=({:.3},{:.3},{:.3})", n.x, n.y, n.z, c.x, c.y, c.z);
	}
}

// ── 既知バグの再現・修正確認テスト ───────────────────────────────

#[test]
/// 旧バージョンで Standard_OutOfRange によりテストランナーごとクラッシュしていた
/// 既知パラメーター (cx=1.0, cy=0.0, cz=1.0, dx=1.0, dy=1.0, dz=1.0) の確認テスト。
/// 現在は正常に処理され、out/stretch_box_known_error_case_1_0_1.step に形状を出力する。
/// 入力・出力ともに単一シェル（閉じた連結面）であることを検証する。
fn stretch_box_known_error_case_1_0_1() {
	let shape = lambda360box();
	assert_eq!(shape.shell_count(), 1, "input shape must have exactly one shell");

	let (cx, cy, cz) = (1.0, 0.0, 1.0);
	let (dx, dy, dz) = (1.0, 1.0, 1.0);

	let result = stretch_ok(&shape, cx, cy, cz, dx, dy, dz);

	match &result {
		Ok(s) => {
			assert_eq!(s.shell_count(), 1, "stretched shape must have exactly one shell");
			write_step(s, "stretch_box_known_error_case_1_0_1");
		}
		Err(e) => println!("Error: {e}"),
	}
	if let Err(e) = result {
		panic!("stretch failed: {e}");
	}
}

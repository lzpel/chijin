use cadrum::{
	utils::{helix_section, revolve_section, stretch_vector},
	Error, SolidTrait, Solid,
};
use glam::DVec3;
use std::panic::{self, AssertUnwindSafe};
use std::path::Path;

fn dvec3(x: f64, y: f64, z: f64) -> DVec3 {
	DVec3::new(x, y, z)
}

fn test_box() -> Vec<Solid> {
	vec![Solid::box_from_corners(
		dvec3(0.0, 0.0, 0.0),
		dvec3(10.0, 10.0, 10.0),
	)]
}

/// テスト用のベース形状として、外部のSTEPファイルを読み込みます。
fn lambda360box() -> Vec<Solid> {
	let mut file = std::fs::File::open(
		"steps/LAMBDA360-BOX-d6cb2eb2d6e0d802095ea1eda691cf9a3e9bf3394301a0d148f53e55f0f97951.step",
	)
	.expect("Failed to open step file");
	cadrum::read_step(&mut file).expect("Failed to read step file")
}

/// 形状をX, Y, Zの各軸方向に順番に引き伸ばします。
fn stretch(
	shape: &Vec<Solid>,
	cx: f64,
	cy: f64,
	cz: f64,
	dx: f64,
	dy: f64,
	dz: f64,
) -> Result<Vec<Solid>, Error> {
	let eps = 1e-10;
	let origin = DVec3::new(cx, cy, cz);
	let after_x: Vec<Solid> = if dx > eps {
		stretch_vector(shape, origin, DVec3::new(dx, 0.0, 0.0))?
	} else {
		shape.clone()
	};
	let after_y: Vec<Solid> = if dy > eps {
		stretch_vector(&after_x, origin, DVec3::new(0.0, dy, 0.0))?
	} else {
		after_x.clone()
	};
	let after_z: Vec<Solid> = if dz > eps {
		stretch_vector(&after_y, origin, DVec3::new(0.0, 0.0, dz))?
	} else {
		after_y.clone()
	};

	after_z.into_iter().map(|s| s.clean()).collect::<Result<Vec<_>, _>>()
}

/// 形状の引き伸ばし処理をパニックキャッチ付きで安全に実行し、結果を返します。
fn stretch_ok(
	shape: &Vec<Solid>,
	cx: f64,
	cy: f64,
	cz: f64,
	dx: f64,
	dy: f64,
	dz: f64,
) -> Result<Vec<Solid>, String> {
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
	fn new(seed: u32) -> Self {
		Self { state: seed }
	}
	fn next_f64(&mut self) -> f64 {
		self.state = self.state.wrapping_mul(1664525).wrapping_add(1013904223);
		(self.state as f64) / (u32::MAX as f64)
	}
	fn gen_range(&mut self, min: f64, max: f64) -> f64 {
		min + self.next_f64() * (max - min)
	}
}

/// 形状を `out/<name>.step` に書き出し、頂点数・三角形数を標準出力に表示します。
fn write_step(shape: &Vec<Solid>, name: &str) {
	std::fs::create_dir_all("out").unwrap();
	let path = format!("out/{name}.step");
	let mut file = std::fs::File::create(&path).unwrap();
	cadrum::write_step(shape, &mut file).expect("STEP write failed");
	let mesh = shape[0].mesh_with_tolerance(0.1).expect("meshing failed");
	assert!(!mesh.vertices.is_empty(), "result shape has no vertices");
	println!(
		"{name}: {} vertices, {} triangles → {path}",
		mesh.vertices.len(),
		mesh.indices.len() / 3,
	);
}

// ==================== stretch_vector ====================

#[test]
fn test_stretch_vector_volume() {
	let shape = test_box();
	// X=5 で切断し +X 方向に 1 引き延ばす → 10×10×11 = 1100
	let result = stretch_vector(&shape, dvec3(5.0, 0.0, 0.0), dvec3(1.0, 0.0, 0.0)).unwrap();
	let v: f64 = result.iter().map(|s| s.volume()).sum();
	assert!((v - 1100.0).abs() < 1e-3, "expected volume ≈ 1100, got {v}");
}

// ==================== revolve_section ====================

#[test]
fn test_revolve_section_volume() {
	let shape = test_box();
	// y=5 の平面（法線=Y）で切断し、Z 軸（x=0,y=5 を通る）周りに全周回転。
	// 断面は x:0..10, z:0..10 の矩形 → 半径 10・高さ 10 の円柱。
	// 期待体積 = π × 10² × 10 = 1000π ≈ 3141.59
	let result = revolve_section(
		&shape,
		dvec3(0.0, 5.0, 0.0),
		dvec3(0.0, 0.0, 1.0),
		dvec3(0.0, 1.0, 0.0),
		std::f64::consts::TAU,
	)
	.unwrap();
	let v: f64 = result.iter().map(|s| s.volume()).sum();

	std::fs::create_dir_all("out").unwrap();
	let mut file = std::fs::File::create("out/revolve_section.step").unwrap();
	cadrum::write_step(&result, &mut file).expect("STEP write failed");

	let expected = std::f64::consts::PI * 10.0f64.powi(2) * 10.0;
	assert!(
		(v - expected).abs() < 1.0,
		"expected volume ≈ {expected:.2}, got {v}"
	);
}

// ==================== helix_section ====================

#[test]
fn test_helix_section_volume() {
	let shape = test_box();
	// y=5 の平面（法線=Y）で切断し、Z 軸周りにヘリカルスイープ。
	// 断面は x:0..10, z:0..10 の矩形（面積=100）。
	// helix_section は align_to_spine=false（断面の向き保持）。
	// Frenet フレームにより回転方向には追従するが初期補正なしのため、
	// 体積 ≈ 2πR × A (回転体のパップス公式) = 2π×5×100 ≈ 3141.59。
	let result = helix_section(
		&shape,
		dvec3(0.0, 5.0, 0.0),
		dvec3(0.0, 0.0, 1.0),
		dvec3(0.0, 1.0, 0.0),
		20.0,
		1.0,
	)
	.unwrap();
	let v: f64 = result.iter().map(|s| s.volume()).sum();

	std::fs::create_dir_all("out").unwrap();
	let mut file = std::fs::File::create("out/helix_section.step").unwrap();
	cadrum::write_step(&result, &mut file).expect("STEP write failed");

	let radius = 5.0;
	let area = 100.0;
	let expected = 2.0 * std::f64::consts::PI * radius * area;
	let tolerance = expected * 0.01;

	println!("helix_section volume: {v:.2}, expected (Pappus revolve): {expected:.2}");
	assert!(
		(v - expected).abs() < tolerance,
		"volume check: expected ≈ {expected:.2}, got {v:.2}"
	);
}

// ==================== stretch (lambda360box) ====================

#[test]
fn diagnose_new_faces() {
	let shape = lambda360box();
	println!(
		"input: face_count={}, shell_count={}",
		shape.iter().flat_map(|s| s.face_iter()).count(),
		shape.iter().map(|s| s.shell_count()).sum::<u32>()
	);

	let origin = DVec3::new(1.0, 0.0, 1.0);
	let delta = DVec3::new(1.0, 0.0, 0.0);

	let half: Vec<Solid> = vec![Solid::half_space(origin, -delta.normalize())];
	let r_half = cadrum::Boolean::intersect(&shape, &half).expect("intersect(half_space) failed");
	println!(
		"  intersect result: tool_face count={}",
		r_half
			.solids
			.iter().flat_map(|s| s.face_iter())
			.filter(|f| r_half.is_tool_face(f))
			.count()
	);

	let big_box: Vec<Solid> = vec![Solid::box_from_corners(
		DVec3::new(-1000.0, -1000.0, -1000.0),
		DVec3::new(1.0, 1000.0, 1000.0),
	)];
	let r_box = cadrum::Boolean::intersect(&shape, &big_box).expect("intersect(big_box) failed");
	println!(
		"  intersect result: tool_face count={}",
		r_box
			.solids
			.iter().flat_map(|s| s.face_iter())
			.filter(|f| r_box.is_tool_face(f))
			.count()
	);
	for (i, face) in r_box
		.solids
		.iter().flat_map(|s| s.face_iter())
		.filter(|f| r_box.is_tool_face(f))
		.enumerate()
	{
		let n = face.normal_at_center();
		let c = face.center_of_mass();
		println!(
			"    face[{i}]: normal=({:.3},{:.3},{:.3}) center=({:.3},{:.3},{:.3})",
			n.x, n.y, n.z, c.x, c.y, c.z
		);
	}
}

#[test]
fn stretch_box_known_error_case_1_0_1() {
	// 旧バージョンで Standard_OutOfRange によりテストランナーごとクラッシュしていた
	// 既知パラメーター (cx=1.0, cy=0.0, cz=1.0, dx=1.0, dy=1.0, dz=1.0) の確認テスト。
	let shape = lambda360box();
	assert_eq!(
		shape.iter().map(|s| s.shell_count()).sum::<u32>(),
		1,
		"input shape must have exactly one shell"
	);

	let result = stretch_ok(&shape, 1.0, 0.0, 1.0, 1.0, 1.0, 1.0);
	match &result {
		Ok(s) => {
			assert_eq!(
				s.iter().map(|s| s.shell_count()).sum::<u32>(),
				1,
				"stretched shape must have exactly one shell"
			);
			write_step(s, "stretch_box_known_error_case_1_0_1");
		}
		Err(e) => println!("Error: {e}"),
	}
	if let Err(e) = result {
		panic!("stretch failed: {e}");
	}
}

#[test]
#[ignore = "3000 試行で約 8 分かかるため通常の cargo test からは除外。実行: cargo test stretch_box_random_survey -- --ignored"]
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

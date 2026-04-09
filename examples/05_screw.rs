use cadrum::{Edge, Error, ProfileOrient, Solid, SolidExt, Transform};
use glam::DVec3;

fn build_m2_screw() -> Result<Vec<Solid>, Error> {
	// iso m2 screw
	let r = 1.0;
	let h_pitch = 0.4;
	let h_thread = 6.0;
	let r_head = 1.75;
	let h_head = 1.3;
	// ISO M ねじの基本三角形高さ H = √3/2 × P (60° 等辺三角形)。
	// 山頂・谷底とも鋭利な fundamental triangle をそのまま用いる
	// (basic profile の山頂 P/8・谷底 P/4 切り詰めは省略)。これにより
	// 谷底半径 = r - r_delta、山頂半径 = r、半フランク角 = atan((P/2)/H) = 30°。
	let r_delta = 3f64.sqrt() / 2.0 * h_pitch;

	// 単一エッジのヘリックス (spine)。半径は谷底に取る。
	// x_ref=DVec3::X を渡しているので、ヘリックスは確定的に (r - r_delta, 0, 0)
	// から始まり、+Z 方向に上昇しつつ +X→+Y→-X→-Y… の順に巻いていく。
	let helix = Edge::helix(r - r_delta, h_pitch, h_thread, DVec3::Z, DVec3::X);

	// 閉じた三角形プロファイル (Vec<Edge> = Wire)。プロファイルはローカル座標で、
	// x が放射方向の突き出し量、y が軸方向 (= ヘリックス始点接線方向)。
	// 外側頂点の x = r_delta なので、sweep 後の山頂は半径 r ぴったりに到達する。
	// polygon は常に閉じる: 最後の点 → 最初の点が自動補完される。
	let profile = Edge::polygon([DVec3::new(0.0, -h_pitch / 2.0, 0.0), DVec3::new(r_delta, 0.0, 0.0), DVec3::new(0.0, h_pitch / 2.0, 0.0)]);

	// プロファイルを XY 平面 (法線 Z) からヘリックス始点の接線方向に
	// 回転し、そのまま始点へ平行移動する。Vec<Edge> は Vec<T: Transform>
	// 経由で align_z / translate を持つ。
	let profile = profile.align_z(helix.start_tangent(), helix.start_point()).translate(helix.start_point());

	// ヘリックスに沿って sweep。Up(helix 軸) は helix では Torsion と等価で
	// 正しいねじ山を作る。
	let thread = Solid::sweep(&profile, &[helix], ProfileOrient::Up(DVec3::Z))?;

	// 三段構成で sharp 三角形プロファイルから ISO 68-1 basic profile (台形) を再現する:
	//   1. sweep で sharp 三角形のねじ山を生成 (主径 r、谷 r - r_delta、平坦なし)
	//   2. union(shaft) で下から H/4 を埋める → 谷底に幅 P/4 の平坦が生まれる
	//   3. intersect(crest) で上から H/8 を削る → 山頂に幅 P/8 の平坦が生まれる
	// 結果: 山頂平坦 P/8、谷底平坦 P/4、フランク全角 60°、ねじ深さ 5H/8 の
	// 厳密な ISO 基本山形。主径 = 2(r - H/8) = 2 - H/4 ≈ 1.913 mm は M2 6g
	// 主径下限 (≈ 1.913 mm) と一致するので、規格内の M2 として通る。
	let shaft = Solid::cylinder(r - r_delta * 6.0 / 8.0, DVec3::Z, h_thread);
	let crest = Solid::cylinder(r - r_delta / 8.0,       DVec3::Z, h_thread);
	let thread_shaft = thread.union([&shaft])?.intersect([&crest])?;

	// 平頭を上に重ねる。
	let head = Solid::cylinder(r_head, DVec3::Z, h_head).translate(DVec3::Z * h_thread);
	thread_shaft.union([&head])
}

fn main() {
	let example_name = std::path::Path::new(file!()).file_stem().unwrap().to_str().unwrap();

	let screw = build_m2_screw().expect("failed to build M2 screw");

	let mut f = std::fs::File::create(format!("{example_name}.step")).expect("failed to create STEP file");
	cadrum::io::write_step(&screw, &mut f).expect("failed to write STEP");
	let mut f_svg = std::fs::File::create(format!("{example_name}.svg")).expect("failed to create SVG file");
	// Helical sweeps produce dense topological edges; hidden lines turn the
	// SVG into a noisy mess, so disable them for this example.
	cadrum::io::write_svg(&screw, DVec3::new(1.0, 1.0, -1.0), 0.5, false, &mut f_svg).expect("failed to write SVG");
	println!("wrote {example_name}.step / {example_name}.svg ({} solids)", screw.len());
}

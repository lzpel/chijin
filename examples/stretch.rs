//! Stretch example: シリンダーを作って中心から XYZ 方向に引き延ばす
//!
//! ```
//! cargo run --example stretch --features bundled
//! ```
//!
//! 出力: out/stretched.brep (BRep テキスト形式)

use chijin::{BooleanShape, Error, Shape};
use glam::DVec3;
use std::path::Path;

/// ツール側フェイスだけを delta 方向に押し出してフィラーを作ります。
fn extrude_tool_faces(result: &BooleanShape, delta: DVec3) -> Result<Shape, Error> {
    let mut filler: Option<Shape> = None;
    for face in result.shape.faces().filter(|f| result.is_tool_face(f)) {
        let extruded = Shape::from(face.extrude(delta)?);
        filler = Some(match filler {
            None => extruded,
            Some(f) => Shape::from(f.union(&extruded)?),
        });
    }
    Ok(filler.unwrap_or_else(Shape::empty))
}

/// 指定された座標とベクトルで形状を分割し、片方を平行移動させた後、隙間を押し出し形状で埋めることで引き伸ばしを行います。
/// intersect の BooleanShape::is_tool_face から切断面を直接取得するため、
/// 法線・重心による heuristic フィルタを使いません。
fn stretch_vector(shape: &Shape, origin: DVec3, delta: DVec3) -> Result<Shape, Error> {
    // Negate so the solid fills the -delta side; intersect then yields part_neg.
    let half = Shape::half_space(origin, -delta.normalize());

    let intersect_result = shape.intersect(&half)?;
    let part_pos = Shape::from(shape.subtract(&half)?).translated(delta);

    let filler = extrude_tool_faces(&intersect_result, delta)?;
    let part_neg = intersect_result.shape;
    let combined = Shape::from(part_neg.union(&filler)?);
    combined.union(&part_pos).map(Shape::from)
}

/// (cx,cy,cz) で切断し、(dx,dy,dz) だけ各軸方向に引き延ばす。
/// delta が 0 以下の軸はスキップする。
fn stretch(shape: Shape, cx: f64, cy: f64, cz: f64, dx: f64, dy: f64, dz: f64) -> Result<Shape, Error> {
    let eps = 1e-10;
    let shape = if dx > eps { stretch_vector(&shape, DVec3::new(cx, 0.0, 0.0), DVec3::new(dx, 0.0, 0.0))? } else { shape };
    let shape = if dy > eps { stretch_vector(&shape, DVec3::new(0.0, cy, 0.0), DVec3::new(0.0, dy, 0.0))? } else { shape };
    let shape = if dz > eps { stretch_vector(&shape, DVec3::new(0.0, 0.0, cz), DVec3::new(0.0, 0.0, dz))? } else { shape };
    shape.clean()
}

fn main() {
    // ── シリンダーを生成 ──────────────────────────────────────
    // 底面中心: 原点 / 軸方向: Z / 半径: 20mm / 高さ: 80mm
    let radius = 20.0_f64;
    let height = 80.0_f64;
    let base = DVec3::ZERO;
    let cylinder = Shape::cylinder(base, radius, DVec3::Z, height);

    // 中心座標（切断位置）
    let center = DVec3::new(0.0, 0.0, height / 2.0);

    // 各軸の伸縮量
    let (dx, dy, dz) = (30.0, 20.0, 40.0);

    println!(
        "シリンダー: 底面中心={base:?}, 半径={radius}mm, 高さ={height}mm"
    );
    println!(
        "切断位置: {center:?} / 伸縮量: X={dx}mm Y={dy}mm Z={dz}mm"
    );

    // ── ストレッチ ────────────────────────────────────────────
    let result = stretch(cylinder, center.x, center.y, center.z, dx, dy, dz)
        .expect("ストレッチに失敗");

    // ── BRep テキストとして書き出し ───────────────────────────
    let out_path = "out/stretched.brep";
    std::fs::create_dir_all(Path::new(out_path).parent().unwrap()).unwrap();
    let mut buf = Vec::new();
    result
        .write_brep_text(&mut buf)
        .expect("BRep 書き込みに失敗");
    std::fs::write(out_path, &buf).expect("ファイル書き込みに失敗");

    // ── メッシュ統計 ──────────────────────────────────────────
    let mesh = result
        .mesh_with_tolerance(0.5)
        .expect("メッシュ生成に失敗");
    println!(
        "完了: {out_path} ({} bytes) — 頂点数: {}, 三角形数: {}",
        buf.len(),
        mesh.vertices.len(),
        mesh.indices.len() / 3,
    );
}

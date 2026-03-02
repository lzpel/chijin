//! Stretch example: シリンダーを作って中心から XYZ 方向に引き延ばす
//!
//! ```
//! cargo run --example stretch --features buildin
//! ```
//!
//! 出力: stretched.brep (BRep テキスト形式)

use chijin::Shape;
use glam::DVec3;

const NORMAL_THRESHOLD: f64 = 0.99;
const COORD_TOLERANCE: f64 = 0.5;

/// 切断面の Face を delta だけ押し出してフィラーソリッドを生成する。
fn extrude_cut_faces(shape: &Shape, axis: usize, cut_coord: f64, delta: f64) -> Shape {
    let extrude_dir = axis_vec(axis, delta);
    let mut filler: Option<Shape> = None;
    for face in shape.faces() {
        let normal = face.normal_at_center();
        let center = face.center_of_mass();
        if axis_component(normal, axis).abs() > NORMAL_THRESHOLD
            && (axis_component(center, axis) - cut_coord).abs() < COORD_TOLERANCE
        {
            let extruded = Shape::from(face.extrude(extrude_dir)).deep_copy();
            filler = Some(match filler {
                None => extruded,
                Some(f) => f.union(&extruded).deep_copy(),
            });
        }
    }
    filler.unwrap_or_else(Shape::empty)
}

/// 1 軸分の切断 → 正側を移動 → ギャップ充填を行う。
fn stretch_axis(shape: Shape, axis: usize, cut_coord: f64, delta: f64) -> Shape {
    let plane_origin = axis_vec(axis, cut_coord);
    let plane_normal = axis_unit(axis);
    let half = Shape::half_space(plane_origin, plane_normal);

    let part_neg = shape.intersect(&half).deep_copy();
    let part_pos = shape.subtract(&half).deep_copy();
    let part_pos = part_pos.translated(axis_vec(axis, delta));

    let filler = extrude_cut_faces(&part_neg, axis, cut_coord, delta);
    part_neg.union(&filler).union(&part_pos).deep_copy()
}

/// (cx,cy,cz) で切断し、(dx,dy,dz) だけ各軸方向に引き延ばす。
/// delta が 0 以下の軸はスキップする。
fn stretch(shape: Shape, cx: f64, cy: f64, cz: f64, dx: f64, dy: f64, dz: f64) -> Shape {
    let eps = 1e-10;
    let shape = if dx > eps { stretch_axis(shape, 0, cx, dx) } else { shape };
    let shape = if dy > eps { stretch_axis(shape, 1, cy, dy) } else { shape };
    let shape = if dz > eps { stretch_axis(shape, 2, cz, dz) } else { shape };
    shape.clean().deep_copy()
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
    let result = stretch(cylinder, center.x, center.y, center.z, dx, dy, dz);

    // ── BRep テキストとして書き出し ───────────────────────────
    let out_path = "stretched.brep";
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

// ── ユーティリティ ────────────────────────────────────────────────

fn axis_vec(axis: usize, v: f64) -> DVec3 {
    match axis {
        0 => DVec3::new(v, 0.0, 0.0),
        1 => DVec3::new(0.0, v, 0.0),
        _ => DVec3::new(0.0, 0.0, v),
    }
}

fn axis_unit(axis: usize) -> DVec3 {
    axis_vec(axis, 1.0)
}

fn axis_component(v: DVec3, axis: usize) -> f64 {
    match axis {
        0 => v.x,
        1 => v.y,
        _ => v.z,
    }
}

//! ちぢん例: 奄美大島の楽器「ちぢん」を chijin ライブラリで再現する
//!
//! ```
//! cargo run --example chijin --features bundled,color
//! ```
//!
//! 出力: out/chijin.step (AP214 STEP, 色付き)

use chijin::{Boolean, Face, Rgb, Shape, Solid};
use glam::DVec3;
use std::f64::consts::PI;
use std::path::Path;

fn main() {
    // ── 色定義 ────────────────────────────────────────────────────────────────
    // 鼓面・胴体: 濃い茶色
    let dark_brown = Rgb { r: 0.36, g: 0.18, b: 0.06 };
    // 縁・締め木: 明るい茶色
    let light_brown = Rgb { r: 0.72, g: 0.48, b: 0.24 };

    // ── 胴体 (cylinder): r=15cm, h=10cm, 原点中心 ────────────────────────────
    // 底面中心を z=-5 にして、形状が z=-5..+5 の範囲に収まるようにする
    let mut cylinder: Solid =Solid::cylinder(DVec3::new(0.0, 0.0, -4.0), 15.0, DVec3::Z, 8.0);
    cylinder.color_paint(dark_brown);

    // ── 縁板 (sheet): x=0 の多角形プロファイルを Z 軸回転体にした薄いリング ──
    // プロファイル点(y, z): (0,5),(15,5),(16,3),(15,4),(0,4)
    // → extrude で厚みを持たせ、回転 (revolution) の代わりに
    //   多数の薄いくさびを union して近似する。
    //
    // 簡易実装: 外径16cm・内径15cmのリングを z=3..5 に配置する薄いシェル
    // outer cylinder - inner cylinder で中空シリンダーを作る
    let sheet_face = Face::from_polygon(&[
        DVec3::new(0.0, 0.0, 5.0),
        DVec3::new(0.0, 15.0, 5.0),
        DVec3::new(0.0, 16.0, 3.0),
        DVec3::new(0.0, 15.0, 4.0),
        DVec3::new(0.0, 0.0, 4.0),
        DVec3::new(0.0, 0.0, 5.0),
    ]).unwrap();
    let mut sheet = sheet_face.revolve(DVec3::ZERO, DVec3::Z, 2.0*PI).unwrap();
    sheet.color_paint(light_brown);
    println!("sheet volume: {}", sheet.volume());

    // ── 締め木 (block): 2cm x 5cm x 1cm ─────────────────────────────────────
    // x軸方向に伸びた板。z軸方向に 60° の仰角で配置し、x=0, y=0, z=15cm へ移動
    // corners: (-1, -2.5, -0.5) .. (1, 2.5, 0.5)
    let block_proto = Solid::box_from_corners(
        DVec3::new(-1.0, -2.5, -0.5),
        DVec3::new(1.0, 2.5, 0.5),
    );
    // z軸まわりに 60°回転（板を斜めにする）してから (0, 0, 15) に移動
    let block_proto = block_proto
        .rotated(DVec3::ZERO, DVec3::Z, 60.0_f64.to_radians())
        .translated(DVec3::new(0.0, 0.0, 15.0));

    // n=12 個の締め木を 360°/n ずつ Z 軸回転して配置
    let n = 12usize;
    let mut blocks: Vec<Solid> = Vec::with_capacity(n);
    for i in 0..n {
        let angle = 2.0 * PI * (i as f64) / (n as f64);
        let mut b = block_proto.rotated(DVec3::ZERO, DVec3::Z, angle);
        b.color_paint(light_brown);
        blocks.push(b);
    }

    // ── すべてを union ───────────────────────────────────────────────────────
    // cylinder + sheet
    let combined: Vec<Solid> = Boolean::union(&[cylinder], &[sheet])
        .expect("cylinder + sheet union に失敗")
        .into();

    // combined + blocks (1 つずつ追加)
    let mut result = combined;
    for (i, block) in blocks.iter().enumerate() {
        result = Boolean::union(&result, std::slice::from_ref(block))
            .unwrap_or_else(|e| panic!("block[{i}] union に失敗: {e:?}"))
            .into();
    }

    // ── STEP ファイルとして書き出し ──────────────────────────────────────────
    let out_path = "out/chijin.step";
    std::fs::create_dir_all(Path::new(out_path).parent().unwrap()).unwrap();
    let mut buf = Vec::new();
    chijin::write_step_with_colors(&result, &mut buf).expect("STEP 書き込みに失敗");
    std::fs::write(out_path, &buf).expect("ファイル書き込みに失敗");

    // ── 統計 ─────────────────────────────────────────────────────────────────
    let mesh = result.mesh_with_tolerance(0.5).expect("メッシュ生成に失敗");
    println!(
        "完了: {out_path} ({} bytes) — 頂点数: {}, 三角形数: {}",
        buf.len(),
        mesh.vertices.len(),
        mesh.indices.len() / 3,
    );
}
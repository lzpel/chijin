use chijin::Shape;
use glam::DVec3;
use std::f64::consts::TAU;
use std::time::{Duration, Instant};

fn theoretical_regions(n: usize) -> usize {
    if n % 2 == 0 {
        (n * n + 2) / 2
    } else {
        (n * n + n + 2) / 2
    }
}

fn ngon_cuts(n: usize) -> Vec<(DVec3, DVec3)> {
    (0..n)
        .map(|k| {
            let angle = TAU * k as f64 / n as f64;
            let origin = DVec3::new(angle.cos(), angle.sin(), 0.0);
            (origin, origin)
        })
        .collect()
}

fn split(shape: &Shape, origin: DVec3, normal: DVec3) -> (Shape, Shape) {
    let hs_pos = Shape::half_space(origin, normal);
    let hs_neg = Shape::half_space(origin, -normal);
    let pos: Shape = shape.intersect(&hs_pos).unwrap().into();
    let neg: Shape = shape.intersect(&hs_neg).unwrap().into();
    (pos, neg)
}

#[test]
fn bench_into_solids() {
    println!();
    println!("【テスト目的】");
    println!("  半径5のシリンダーを正n角形由来のハーフスペース平面で順次分割し、");
    println!("  into_solids/from_solids の往復コストと boolean 演算速度への影響を計測する。");
    println!();
    println!("【パターン】");
    println!("  A: 毎ステップ compound のまま boolean 演算（into_solids を呼ばない）");
    println!("  B: 毎ステップ from_solids→compound→split→into_solids→Vec<Solid> を往復");
    println!();
    println!("【列の説明】");
    println!("  A(ms)       : Pattern A の n ステップ合計時間");
    println!("  B split(ms) : Pattern B の boolean 演算部分の合計時間");
    println!("  B over(ms)  : Pattern B の from_solids + into_solids のみにかかった合計時間");
    println!("  theory      : 平面配置の理論上の分割数（交点が全てシリンダー内にある場合）");
    println!("  A shells    : Pattern A の最終 compound の shell 数（solid 数の近似）");
    println!("  B len       : Pattern B の最終 Vec<Solid> の要素数");
    println!();
    println!(
        "{:>4}  {:>10}  {:>10}  {:>10}  {:>8}  {:>8}  {:>8}",
        "n", "A(ms)", "B split(ms)", "B over(ms)", "theory", "A shells", "B len"
    );

    for n in 3..=10usize {
        let cuts = ngon_cuts(n);

        // --- Pattern A: compound のまま分割（into_solids なし） ---
        let cylinder_a = Shape::cylinder(DVec3::ZERO, 5.0, DVec3::Z, 10.0);
        let mut compound_a = Shape::from_solids(vec![cylinder_a]);

        let t_a = Instant::now();
        for &(origin, normal) in &cuts {
            let (pos, neg) = split(&compound_a, origin, normal);
            compound_a = Shape::from_solids(vec![pos, neg]);
        }
        let time_a = t_a.elapsed();

        // --- Pattern B: Vec<Shape(Solid)> ↔ compound を毎回往復 ---
        let cylinder_b = Shape::cylinder(DVec3::ZERO, 5.0, DVec3::Z, 10.0);
        let mut solids_b: Vec<Shape> = vec![cylinder_b];
        let mut time_b_split = Duration::ZERO;
        let mut time_b_overhead = Duration::ZERO;

        for &(origin, normal) in &cuts {
            let t0 = Instant::now();
            let compound = Shape::from_solids(solids_b);
            time_b_overhead += t0.elapsed();

            let t1 = Instant::now();
            let (pos, neg) = split(&compound, origin, normal);
            time_b_split += t1.elapsed();

            let t2 = Instant::now();
            let mut new_solids = pos.into_solids();
            new_solids.extend(neg.into_solids());
            solids_b = new_solids;
            time_b_overhead += t2.elapsed();
        }

        println!(
            "{:>4}  {:>10}  {:>10}  {:>10}  {:>8}  {:>8}  {:>8}",
            n,
            time_a.as_millis(),
            time_b_split.as_millis(),
            time_b_overhead.as_millis(),
            theoretical_regions(n),
            compound_a.shell_count(),
            solids_b.len()
        );
    }
}

use cadrum::{Solid, SolidExt};
use glam::DVec3;
use std::time::{Duration, Instant};

fn make_toruses(offset: DVec3) -> Vec<Solid> {
	(0..10).flat_map(|i| (0..10).map(move |j| Solid::torus(5.0, 1.0, DVec3::Z).translate(DVec3::new(i as f64 * 30.0, j as f64 * 30.0, 0.0) + offset))).collect()
}

fn bboxes_overlap([amin, amax]: [DVec3; 2], [bmin, bmax]: [DVec3; 2]) -> bool {
	amin.x <= bmax.x && amax.x >= bmin.x && amin.y <= bmax.y && amax.y >= bmin.y && amin.z <= bmax.z && amax.z >= bmin.z
}

/// A グループから B グループ（offset 位置）を pairwise subtract する。
/// optimized=true のとき bbox で非交差ペアをスキップ。
/// 戻り値: (経過時間, subtract 結果の Solid 一覧)
fn run_subtract(offset: DVec3, optimized: bool) -> (Duration, Vec<Solid>) {
	let a = make_toruses(DVec3::ZERO);
	let b = make_toruses(offset);
	let t0 = Instant::now();
	let (results, skipped) = if !optimized {
		(a.subtract(&b).unwrap(), 0)
	} else {
		let bboxes_b: Vec<[DVec3; 2]> = b.iter().map(|s| s.bounding_box()).collect();

		let mut results: Vec<Solid> = Vec::new();
		let mut skipped = 0u32;

		for sa in &a {
			let bb_a = sa.bounding_box();
			let tools: Vec<&Solid> = b.iter().zip(&bboxes_b).filter(|(_sb, &bb_b)| bboxes_overlap(bb_a, bb_b)).map(|(sb, _)| sb).collect();
			if tools.is_empty() {
				skipped += 1;
				results.push(sa.clone());
			} else {
				let r = vec![sa.clone()].subtract(tools.iter().copied()).unwrap();
				results.extend(r);
			}
		}
		(results, skipped)
	};

	let elapsed = t0.elapsed();
	println!("  optimized={optimized}: {elapsed:?}  skipped={skipped}  results={}", results.len());
	(elapsed, results)
}

#[test]
fn test_subtract_bbox_speedup() {
	// non-intersecting case: offset=(15,15,0) A and B are completely separated
	println!("[non-intersecting offset=(15,15,0)]");
	let (no_bbox, no_bbox_solids) = run_subtract(DVec3::new(15.0, 15.0, 0.0), false);
	let (bbox, bbox_solids) = run_subtract(DVec3::new(15.0, 15.0, 0.0), true);
	println!("no_bbox_solids.volume(): {}", no_bbox_solids.iter().map(|s| s.volume()).sum::<f64>());
	println!("bbox_solids.volume(): {}", bbox_solids.iter().map(|s| s.volume()).sum::<f64>());
	let speedup = no_bbox.as_secs_f64() / bbox.as_secs_f64();
	println!("  -> speedup: {speedup:.1}x\n");

	// partially-intersecting case: offset=(3,3,0) 100 pairs of the same index intersect
	// 9900 non-intersecting pairs can be skipped by bbox, but 100 intersecting pairs require heavy computation
	println!("[partially-intersecting offset=(3,3,0)]");
	let (no_bbox2, no_bbox2_solids) = run_subtract(DVec3::new(3.0, 3.0, 0.0), false);
	let (bbox2, bbox2_solids) = run_subtract(DVec3::new(3.0, 3.0, 0.0), true);
	println!("no_bbox2.volume(): {}", no_bbox2_solids.iter().map(|s| s.volume()).sum::<f64>());
	println!("bbox2.volume(): {}", bbox2_solids.iter().map(|s| s.volume()).sum::<f64>());
	let speedup2 = no_bbox2.as_secs_f64() / bbox2.as_secs_f64();
	println!("  -> speedup: {speedup2:.1}x\n");
}
/*
running 1 test
[non-intersecting offset=(15,15,0)]
  optimized=false: 24.9829ms  skipped=0  results=100
  optimized=true: 1.7564ms  skipped=100  results=100
no_bbox_solids.volume(): 9869.604401089357
bbox_solids.volume(): 9869.604401089357
  -> speedup: 14.2x

[partially-intersecting offset=(3,3,0)]
test test_subtract_bbox_speedup has been running for over 60 seconds
  optimized=false: 71.8750439s  skipped=0  results=200
  optimized=true: 66.1344749s  skipped=0  results=200
no_bbox2.volume(): 8465.843298299309
bbox2.volume(): 8465.843298299309
  -> speedup: 1.1x

  */

// i think this kind of optimization is not worth it

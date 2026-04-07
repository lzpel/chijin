use cadrum::{Boolean, Solid};
use glam::DVec3;

#[test]
fn test_union_cylinders() {
	// 互いに自己交差している円柱

	let a = Solid::cylinder(1.1, DVec3::Z, 1.0).translate(DVec3::new(1.0, 0.0, 0.0));
	let b = Solid::cylinder(1.1, DVec3::Z, 1.0).translate(DVec3::new(-1.0, 0.0, 0.0));
	let c = Solid::cylinder(1.1, DVec3::Z, 1.0).translate(DVec3::new(0.0, 1.0, 0.0));
	let d = Solid::cylinder(1.1, DVec3::Z, 1.0).translate(DVec3::new(0.0, -1.0, 0.0));

	let union_a_b = Boolean::union(&[a.clone()], &[b.clone()]).unwrap();
	println!("union([A], [B]) solid count: {}", union_a_b.solids().len());

	let union_ab_cd = Boolean::union(&[a.clone(), b.clone()], &[c.clone(), d.clone()]).unwrap();
	println!("union([A, B], [C, D]) solid count: {}", union_ab_cd.solids().len());

	let all = vec![a, b, c, d];
	let union_all_all = Boolean::union(&all, &all).unwrap();
	println!("union([ABCD], [ABCD]) solid count: {}", union_all_all.solids().len());

	//
}

#[test]
fn test_union_shifted() {
	// ユーザー指定の座標 (AとBの距離=2.0なので実はr=1.1だと自己交差する)
	let a = Solid::cylinder(1.1, DVec3::Z, 1.0);
	let b = Solid::cylinder(1.1, DVec3::Z, 1.0).translate(DVec3::new(2.0, 0.0, 0.0));
	let c = Solid::cylinder(1.1, DVec3::Z, 1.0).translate(DVec3::new(0.0, 1.0, 0.0));
	let d = Solid::cylinder(1.1, DVec3::Z, 1.0).translate(DVec3::new(2.0, 1.0, 0.0));

	let union_ab_cd = Boolean::union(&[a.clone(), b.clone()], &[c.clone(), d.clone()]).unwrap();
	println!("union([A(0,0), B(2,0)], [C(0,1), D(2,1)]) solid count: {}", union_ab_cd.solids().len());

	// 完全にグループ内が自己交差しない座標 (AとBの距離=4.0 > 2.2)
	let a_sep = Solid::cylinder(1.1, DVec3::Z, 1.0);
	let b_sep = Solid::cylinder(1.1, DVec3::Z, 1.0).translate(DVec3::new(4.0, 0.0, 0.0));
	let c_sep = Solid::cylinder(1.1, DVec3::Z, 1.0).translate(DVec3::new(0.0, 1.0, 0.0));
	let d_sep = Solid::cylinder(1.1, DVec3::Z, 1.0).translate(DVec3::new(4.0, 1.0, 0.0));

	let union_sep = Boolean::union(&[a_sep.clone(), b_sep.clone()], &[c_sep.clone(), d_sep.clone()]).unwrap();
	println!("union([A(0,0), B(4.0,0)], [C(0,1), D(4.0,1)]) solid count: {}", union_sep.solids().len());

	// 結論をプリントする
	println!("\n=== 結論 (Conclusion) ===");
	println!("距離が2.0の場合: 引数(A,B)の配列内ですでに自己交差しているため、1つのSolidに自動結合されず2個のままになります。");
	println!("距離が4.0の場合: 配列内で自己交差がないため、A-CとB-Dの重なりが正しく結合され、2個のSolid(ACの融合体, BDの融合体)になります。");
	println!("つまり、OpenCASCADEの Boolean演算(Fuse) は「グループ間の交差」のみを融合し、「グループ内部(同一配列内)の交差」は無視して結合してくれません。");
	println!("複数の重なるソリッドを結合したい場合は、内部で自己交差のない引数を作るか、事前結合(Sewing/逐次Fuseなど)が必要です。");
	println!("==========================\n");
}

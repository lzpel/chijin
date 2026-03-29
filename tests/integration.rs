//! Integration tests for cadrum - OpenCASCADE Rust bindings.
//!
//! These tests correspond to acceptance criteria T-01 through T-08
//! defined in 仕様書.md §4.3.

use cadrum::{Shape, Solid};
use glam::DVec3;

fn dvec3(x: f64, y: f64, z: f64) -> DVec3 {
	DVec3::new(x, y, z)
}

fn test_box() -> Vec<Solid> {
	vec![Solid::box_from_corners(dvec3(0.0, 0.0, 0.0), dvec3(10.0, 10.0, 10.0))]
}

fn test_box_2() -> Vec<Solid> {
	vec![Solid::box_from_corners(dvec3(5.0, 5.0, 5.0), dvec3(15.0, 15.0, 15.0))]
}

fn test_box_3() -> Vec<Solid> {
	vec![Solid::box_from_corners(dvec3(3.0, 3.0, 3.0), dvec3(8.0, 8.0, 8.0))]
}

/// Helper: write shape to BRep binary bytes
fn shape_to_brep_bytes(shape: &[Solid]) -> Vec<u8> {
	let mut buf = Vec::new();
	cadrum::write_brep_bin(shape, &mut buf).unwrap();
	buf
}

// ==================== T-01: Boolean drop order safety ====================

#[test]
fn test_t01_union_drop_result_first() {
	let a = test_box();
	let b = test_box_2();
	let result = cadrum::Boolean::union(&a, &b).unwrap();
	drop(result);
	drop(a);
	drop(b);
}

#[test]
fn test_t01_union_drop_result_last() {
	let a = test_box();
	let b = test_box_2();
	let result = cadrum::Boolean::union(&a, &b).unwrap();
	drop(a);
	drop(b);
	drop(result);
}

#[test]
fn test_t01_subtract_drop_order() {
	let a = test_box();
	let b = test_box_2();
	let result = cadrum::Boolean::subtract(&a, &b).unwrap();
	drop(a);
	drop(b);
	drop(result);
}

#[test]
fn test_t01_intersect_drop_order() {
	let a = test_box();
	let b = test_box_2();
	let result = cadrum::Boolean::intersect(&a, &b).unwrap();
	drop(a);
	drop(b);
	drop(result);
}

#[test]
fn test_t01_chained_boolean_drops() {
	let a = test_box();
	let b = test_box_2();
	let c = test_box_3();
	let r1 = cadrum::Boolean::union(&a, &b).unwrap();
	let r2 = cadrum::Boolean::subtract(&r1.solids, &c).unwrap();
	drop(r1);
	drop(r2);
	drop(a);
	drop(b);
	drop(c);
}

// ==================== T-02: read multiple times ====================

#[test]
fn test_t02_multiple_reads_no_crash() {
	let original = test_box();
	let brep_data = shape_to_brep_bytes(&original);
	for _ in 0..5 {
		let _shape = cadrum::read_brep_bin(&mut brep_data.as_slice()).unwrap();
	}
}

// ==================== T-03: Mesh normals count ====================

#[test]
fn test_t03_mesh_normals_count() {
	let shape = test_box();
	let mesh = shape.mesh_with_tolerance(0.1).unwrap();
	assert_eq!(mesh.normals.len(), mesh.vertices.len());
}

// ==================== T-04: Approximation tolerance ====================

#[test]
fn test_t04_approximation_tolerance() {
	let cyl: Vec<Solid> = vec![Solid::cylinder(dvec3(0.0, 0.0, 0.0), 10.0, dvec3(0.0, 0.0, 1.0), 20.0)];
	let mut has_difference = false;
	for edge in cyl.edges() {
		let coarse = edge.approximation_segments(1.0).count();
		let fine = edge.approximation_segments(0.01).count();
		if fine > coarse {
			has_difference = true;
		}
	}
	assert!(
		has_difference,
		"Fine tolerance should produce more points than coarse"
	);
}

// ==================== T-05: Translation on compound shapes ====================

#[test]
fn test_t05_translated_compound() {
	let a = test_box();
	let b = test_box_2();
	let compound: Vec<Solid> = cadrum::Boolean::union(&a, &b).unwrap().into();
	let v = dvec3(100.0, 0.0, 0.0);
	let orig_mesh = compound.clone().mesh_with_tolerance(0.1).unwrap();
	let shifted = compound.translate(v);
	let shifted_mesh = shifted.mesh_with_tolerance(0.1).unwrap();

	assert_eq!(orig_mesh.vertices.len(), shifted_mesh.vertices.len());
	for (o, s) in orig_mesh.vertices.iter().zip(shifted_mesh.vertices.iter()) {
		assert!((s.x - o.x - v.x).abs() < 1e-6);
		assert!((s.y - o.y - v.y).abs() < 1e-6);
		assert!((s.z - o.z - v.z).abs() < 1e-6);
	}
}

// ==================== T-06: BRep binary roundtrip ====================

#[test]
fn test_t06_brep_roundtrip() {
	let original = test_box();
	let orig_mesh = original.mesh_with_tolerance(0.1).unwrap();

	let brep_data = shape_to_brep_bytes(&original);
	let restored = cadrum::read_brep_bin(&mut brep_data.as_slice()).unwrap();
	let rest_mesh = restored.mesh_with_tolerance(0.1).unwrap();

	assert_eq!(orig_mesh.vertices.len(), rest_mesh.vertices.len());
	for (o, r) in orig_mesh.vertices.iter().zip(rest_mesh.vertices.iter()) {
		assert!((o.x - r.x).abs() < 1e-10);
		assert!((o.y - r.y).abs() < 1e-10);
		assert!((o.z - r.z).abs() < 1e-10);
	}
}

// ==================== T-07: No temporary files ====================

#[test]
fn test_t07_stream_api_only() {
	let shape = test_box();
	let data = shape_to_brep_bytes(&shape);
	assert!(!data.is_empty());
	let _restored = cadrum::read_brep_bin(&mut data.as_slice()).unwrap();
}

// ==================== T-08: Boolean returns BooleanShape, convertible to Shape ====================

#[test]
fn test_t08_boolean_returns_shape() {
	let a = test_box();
	let b = test_box_2();
	let _union: Vec<Solid> = cadrum::Boolean::union(&a, &b).unwrap().into();
	let _sub: Vec<Solid> = cadrum::Boolean::subtract(&a, &b).unwrap().into();
	let _inter: Vec<Solid> = cadrum::Boolean::intersect(&a, &b).unwrap().into();
}

// ==================== STEP export ====================

#[test]
fn test_hollow_cube_write_step() {
	let outer: Vec<Solid> = vec![Solid::box_from_corners(dvec3(-10.0, -10.0, -10.0), dvec3(10.0, 10.0, 10.0))];
	let inner: Vec<Solid> = vec![Solid::box_from_corners(dvec3(-5.0, -5.0, -5.0), dvec3(5.0, 5.0, 5.0))];
	let hollow_cube: Vec<Solid> = cadrum::Boolean::subtract(&outer, &inner).unwrap().into();

	std::fs::create_dir_all("out").unwrap();
	let mut file = std::fs::File::create("out/hollow_cube.step").unwrap();
	cadrum::write_step(&hollow_cube, &mut file).unwrap();
}

// ==================== Additional Tests ====================

#[test]
fn test_empty_shape() {
	let empty: Vec<Solid> = vec![];
	assert!(empty.is_null());
}

#[test]
fn test_deep_copy() {
	let original = test_box();
	let copy = original.clone();
	drop(original);
	assert!((copy.volume() - 1000.0).abs() < 1e-6);
}

#[test]
fn test_edge_iteration() {
	let shape = test_box();
	assert!((shape.volume() - 1000.0).abs() < 1e-6);
}

#[test]
fn test_half_space_intersect() {
	let shape = test_box();
	let half: Vec<Solid> = vec![Solid::half_space(dvec3(5.0, 0.0, 0.0), dvec3(1.0, 0.0, 0.0))];
	let result = cadrum::Boolean::intersect(&shape, &half).unwrap();
	assert!(!result.solids.is_null());
}

#[test]
fn test_cylinder() {
	let cyl: Vec<Solid> = vec![Solid::cylinder(dvec3(0.0, 0.0, 0.0), 5.0, dvec3(0.0, 0.0, 1.0), 10.0)];
	let expected = std::f64::consts::PI * 5.0f64.powi(2) * 10.0;
	assert!((cyl.volume() - expected).abs() < 1e-6);
}

#[test]
fn test_brep_text_roundtrip() {
	let original = test_box();

	let mut text_data = Vec::new();
	cadrum::write_brep_text(&original, &mut text_data).unwrap();
	assert!(!text_data.is_empty());

	let restored = cadrum::read_brep_text(&mut text_data.as_slice()).unwrap();
	let orig_mesh = original.mesh_with_tolerance(0.1).unwrap();
	let rest_mesh = restored.mesh_with_tolerance(0.1).unwrap();
	assert_eq!(orig_mesh.vertices.len(), rest_mesh.vertices.len());
}

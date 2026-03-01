//! Integration tests for Chijin - OpenCASCADE Rust bindings.
//!
//! These tests correspond to acceptance criteria T-01 through T-08
//! defined in 仕様書.md §4.3.

use chijin::{Mesh, Shape};
use glam::DVec3;

fn dvec3(x: f64, y: f64, z: f64) -> DVec3 {
	DVec3::new(x, y, z)
}

fn test_box() -> Shape {
	Shape::box_from_corners(dvec3(0.0, 0.0, 0.0), dvec3(10.0, 10.0, 10.0))
}

fn test_box_2() -> Shape {
	Shape::box_from_corners(dvec3(5.0, 5.0, 5.0), dvec3(15.0, 15.0, 15.0))
}

fn test_box_3() -> Shape {
	Shape::box_from_corners(dvec3(3.0, 3.0, 3.0), dvec3(8.0, 8.0, 8.0))
}

/// Helper: write shape to BRep binary bytes
fn shape_to_brep_bytes(shape: &Shape) -> Vec<u8> {
	let mut buf = Vec::new();
	shape.write_brep_bin(&mut buf).unwrap();
	buf
}

// ==================== T-01: Boolean drop order safety ====================

#[test]
fn test_t01_union_drop_result_first() {
	let a = test_box();
	let b = test_box_2();
	let result = a.union(&b);
	drop(result);
	drop(a);
	drop(b);
}

#[test]
fn test_t01_union_drop_result_last() {
	let a = test_box();
	let b = test_box_2();
	let result = a.union(&b);
	drop(a);
	drop(b);
	drop(result);
}

#[test]
fn test_t01_subtract_drop_order() {
	let a = test_box();
	let b = test_box_2();
	let result = a.subtract(&b);
	drop(a);
	drop(b);
	drop(result);
}

#[test]
fn test_t01_intersect_drop_order() {
	let a = test_box();
	let b = test_box_2();
	let result = a.intersect(&b);
	drop(a);
	drop(b);
	drop(result);
}

#[test]
fn test_t01_chained_boolean_drops() {
	let a = test_box();
	let b = test_box_2();
	let c = test_box_3();
	let r1 = a.union(&b);
	let r2 = r1.subtract(&c);
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
		let _shape = Shape::read_brep_bin(&mut brep_data.as_slice()).unwrap();
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
	let cyl = Shape::cylinder(dvec3(0.0, 0.0, 0.0), 10.0, dvec3(0.0, 0.0, 1.0), 20.0);
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
	let compound = a.union(&b);
	let v = dvec3(100.0, 0.0, 0.0);
	let shifted = compound.translated(v);

	let orig_mesh = compound.mesh_with_tolerance(0.1).unwrap();
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
	let restored = Shape::read_brep_bin(&mut brep_data.as_slice()).unwrap();
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
	let _restored = Shape::read_brep_bin(&mut data.as_slice()).unwrap();
}

// ==================== T-08: Boolean returns Shape ====================

#[test]
fn test_t08_boolean_returns_shape() {
	let a = test_box();
	let b = test_box_2();
	let _union: Shape = a.union(&b);
	let _sub: Shape = a.subtract(&b);
	let _inter: Shape = a.intersect(&b);
}

// ==================== Additional Tests ====================

#[test]
fn test_empty_shape() {
	let empty = Shape::empty();
	assert!(!empty.is_null());
}

#[test]
fn test_deep_copy() {
	let original = test_box();
	let copy = original.deep_copy();
	drop(original);
	let mesh = copy.mesh_with_tolerance(0.1).unwrap();
	assert!(!mesh.vertices.is_empty());
}

#[test]
fn test_face_iteration() {
	let shape = test_box();
	let faces: Vec<_> = shape.faces().collect();
	assert_eq!(faces.len(), 6);
	for face in &faces {
		let normal = face.normal_at_center();
		let len = normal.length();
		assert!((len - 1.0).abs() < 1e-6);
	}
}

#[test]
fn test_edge_iteration() {
	let shape = test_box();
	let edges: Vec<_> = shape.edges().collect();
	assert_eq!(edges.len(), 12);
}

#[test]
fn test_face_extrude() {
	let shape = test_box();
	let face = shape.faces().next().unwrap();
	let solid = face.extrude(dvec3(0.0, 0.0, 5.0));
	let extruded: Shape = solid.into();
	let mesh = extruded.mesh_with_tolerance(0.1).unwrap();
	assert!(!mesh.vertices.is_empty());
}

#[test]
fn test_half_space_intersect() {
	let shape = test_box();
	let half = Shape::half_space(dvec3(5.0, 0.0, 0.0), dvec3(1.0, 0.0, 0.0));
	let result = shape.intersect(&half);
	assert!(!result.is_null());
}

#[test]
fn test_cylinder() {
	let cyl = Shape::cylinder(dvec3(0.0, 0.0, 0.0), 5.0, dvec3(0.0, 0.0, 1.0), 10.0);
	let mesh = cyl.mesh_with_tolerance(0.1).unwrap();
	assert!(!mesh.vertices.is_empty());
	assert_eq!(mesh.normals.len(), mesh.vertices.len());
}

#[test]
fn test_brep_text_roundtrip() {
	let original = test_box();

	let mut text_data = Vec::new();
	original.write_brep_text(&mut text_data).unwrap();
	assert!(!text_data.is_empty());

	let restored = Shape::read_brep_text(&mut text_data.as_slice()).unwrap();
	let orig_mesh = original.mesh_with_tolerance(0.1).unwrap();
	let rest_mesh = restored.mesh_with_tolerance(0.1).unwrap();
	assert_eq!(orig_mesh.vertices.len(), rest_mesh.vertices.len());
}

use chijin::{Face, Shape, Solid};
use glam::DVec3;

fn dvec3(x: f64, y: f64, z: f64) -> DVec3 {
	DVec3::new(x, y, z)
}

#[test]
fn test_face_iteration() {
	let shape: Vec<Solid> = vec![Solid::box_from_corners(dvec3(0.0, 0.0, 0.0), dvec3(10.0, 10.0, 10.0))];
	let faces: Vec<_> = shape.faces().collect();
	assert_eq!(faces.len(), 6);
	for face in &faces {
		let normal = face.normal_at_center();
		assert!((normal.length() - 1.0).abs() < 1e-6);
	}
}

#[test]
fn test_face_from_polygon() {
	// Unit square in the XZ plane (y=0); normal should point toward +Y.
	let face = Face::from_polygon(&[
		dvec3(0.0, 0.0, 0.0),
		dvec3(1.0, 0.0, 0.0),
		dvec3(1.0, 0.0, 1.0),
		dvec3(0.0, 0.0, 1.0),
	])
	.unwrap();
	let normal = face.normal_at_center();
	assert!((normal.length() - 1.0).abs() < 1e-6);
}

#[test]
fn test_face_extrude() {
	// Known face: unit square in XZ plane, extruded along +Y.
	let face = Face::from_polygon(&[
		dvec3(0.0, 0.0, 0.0),
		dvec3(1.0, 0.0, 0.0),
		dvec3(1.0, 0.0, 1.0),
		dvec3(0.0, 0.0, 1.0),
	])
	.unwrap();
	let solid = face.extrude(dvec3(0.0, 1.0, 0.0)).unwrap();
	let shape: Vec<Solid> = vec![solid];
	assert_eq!(shape.shell_count(), 1);
}

#[test]
fn test_face_revolve() {
	// Rectangle in XZ plane (x: 1..3, z: 0..2), revolved around Z axis.
	// The face does not cross the axis, so no self-intersection.
	let face = Face::from_polygon(&[
		dvec3(1.0, 0.0, 0.0),
		dvec3(3.0, 0.0, 0.0),
		dvec3(3.0, 0.0, 2.0),
		dvec3(1.0, 0.0, 2.0),
	])
	.unwrap();
	let solid = face
		.revolve(DVec3::ZERO, dvec3(0.0, 0.0, 1.0), std::f64::consts::TAU)
		.unwrap();
	let shape: Vec<Solid> = vec![solid];
	assert_eq!(shape.shell_count(), 1);
}

#[test]
fn test_face_helix_pappus() {
	// 1×1 square at x=5 (centroid at (5,0,0.5), radius=5 from Z axis).
	// Helix: pitch=10, turns=1 → height=10.
	// Pappus: volume = area × path_length
	//   path_length = sqrt((2π×5)² + 10²) ≈ 32.97
	//   expected volume ≈ 1.0 × 32.97
	let face = Face::from_polygon(&[
		dvec3(4.5, -0.5, 0.0),
		dvec3(5.5, -0.5, 0.0),
		dvec3(5.5, 0.5, 0.0),
		dvec3(4.5, 0.5, 0.0),
	])
	.unwrap();
	let solid = face
		.helix(DVec3::ZERO, dvec3(0.0, 0.0, 1.0), 10.0, 1.0, true)
		.unwrap();
	let shape: Vec<Solid> = vec![solid];
	std::fs::create_dir_all("out").unwrap();
	let mut file = std::fs::File::create("out/helix_test.step").unwrap();
	chijin::write_step(&shape, &mut file).expect("STEP write failed");

	let v = shape.volume();
	println!("helix volume: {v:.4}");

	let radius = 5.0;
	let path_length =
		((2.0 * std::f64::consts::PI * radius).powi(2) + 10.0f64.powi(2)).sqrt();
	let expected = 1.0 * path_length;
	let tolerance = expected * 0.10;

	println!("helix volume: {v:.4}, expected (Pappus): {expected:.4}, diff: {:.1}%",
		(v - expected).abs() / expected * 100.0);
	assert!(
		(v - expected).abs() < tolerance,
		"Pappus volume check: expected ≈ {expected:.2}, got {v:.2}"
	);
}

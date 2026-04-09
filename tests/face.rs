use cadrum::{Face, Solid};
use glam::DVec3;

fn dvec3(x: f64, y: f64, z: f64) -> DVec3 {
	DVec3::new(x, y, z)
}

#[test]
fn test_face_iteration() {
	let shape = [Solid::cube(10.0, 10.0, 10.0)];
	let faces: Vec<_> = shape.iter().flat_map(|s| s.face_iter()).collect();
	assert_eq!(faces.len(), 6);
	for face in &faces {
		let normal = face.normal_at_center();
		assert!((normal.length() - 1.0).abs() < 1e-6);
	}
}

#[test]
fn test_face_from_polygon() {
	// Unit square in the XZ plane (y=0); normal should point toward +Y.
	let face = Face::from_polygon(&[dvec3(0.0, 0.0, 0.0), dvec3(1.0, 0.0, 0.0), dvec3(1.0, 0.0, 1.0), dvec3(0.0, 0.0, 1.0)]).unwrap();
	let normal = face.normal_at_center();
	assert!((normal.length() - 1.0).abs() < 1e-6);
}

#[test]
fn test_face_extrude() {
	// Known face: unit square in XZ plane, extruded along +Y.
	let face = Face::from_polygon(&[dvec3(0.0, 0.0, 0.0), dvec3(1.0, 0.0, 0.0), dvec3(1.0, 0.0, 1.0), dvec3(0.0, 0.0, 1.0)]).unwrap();
	let solid = face.extrude(dvec3(0.0, 1.0, 0.0)).unwrap();
	let shape: Vec<Solid> = vec![solid];
	assert_eq!(shape.iter().map(|s| s.shell_count()).sum::<u32>(), 1);
}

#[test]
fn test_face_revolve() {
	// Rectangle in XZ plane (x: 1..3, z: 0..2), revolved around Z axis.
	// The face does not cross the axis, so no self-intersection.
	let face = Face::from_polygon(&[dvec3(1.0, 0.0, 0.0), dvec3(3.0, 0.0, 0.0), dvec3(3.0, 0.0, 2.0), dvec3(1.0, 0.0, 2.0)]).unwrap();
	let solid = face.revolve(DVec3::ZERO, dvec3(0.0, 0.0, 1.0), std::f64::consts::TAU).unwrap();
	let shape: Vec<Solid> = vec![solid];
	assert_eq!(shape.iter().map(|s| s.shell_count()).sum::<u32>(), 1);
}


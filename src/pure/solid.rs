use glam::{DAffine3, DMat3, DVec2, DVec3};
use std::f64::consts::PI;

use crate::common::error::Error;
use crate::common::mesh::Mesh;
use crate::traits::SolidTrait;
use super::edge::Edge;
use super::face::Face;

// ==================== Primitive definition ====================

/// Analytical primitive in local (untransformed) coordinates.
#[derive(Debug, Clone)]
enum Primitive {
	/// Axis-aligned box defined by two corners.
	Box { min: DVec3, max: DVec3 },
	/// Sphere at origin with given radius.
	Sphere { radius: f64 },
	/// Cylinder: base at origin, axis along +Z, radius r, height h.
	Cylinder { radius: f64, height: f64 },
	/// Cone: base at origin, axis along +Z, bottom radius r1, top radius r2, height h.
	Cone { r1: f64, r2: f64, height: f64 },
	/// Torus: center at origin, axis along +Z, major radius R, minor radius r.
	Torus { major_radius: f64, minor_radius: f64 },
	/// Null / empty solid.
	Null,
}

// ==================== Solid ====================

/// A solid in the pure Rust backend.
///
/// Stores an analytical primitive definition plus an affine transform.
/// All queries (volume, bbox, contains, etc.) are computed analytically.
#[derive(Debug, Clone)]
pub struct Solid {
	primitive: Primitive,
	/// World = transform * local
	transform: DAffine3,
}

impl Solid {
	fn new(primitive: Primitive, transform: DAffine3) -> Self {
		Solid { primitive, transform }
	}

	/// Returns `true` if this is a null/empty solid.
	pub fn is_null(&self) -> bool {
		matches!(self.primitive, Primitive::Null)
	}

	/// Compute the scale factor (determinant of the 3x3 part) of the transform.
	fn scale_det(&self) -> f64 {
		self.transform.matrix3.determinant()
	}

	/// Local-space volume of the primitive (before transform).
	fn local_volume(&self) -> f64 {
		match &self.primitive {
			Primitive::Box { min, max } => {
				let d = *max - *min;
				d.x * d.y * d.z
			}
			Primitive::Sphere { radius } => {
				4.0 / 3.0 * PI * radius.powi(3)
			}
			Primitive::Cylinder { radius, height } => {
				PI * radius.powi(2) * height
			}
			Primitive::Cone { r1, r2, height } => {
				PI * height / 3.0 * (r1 * r1 + r1 * r2 + r2 * r2)
			}
			Primitive::Torus { major_radius, minor_radius } => {
				2.0 * PI * PI * major_radius * minor_radius.powi(2)
			}
			Primitive::Null => 0.0,
		}
	}

	/// Get the 8 corners of the local-space AABB, then transform them.
	fn transformed_aabb(&self) -> [DVec3; 2] {
		let (lmin, lmax) = self.local_aabb();
		let corners = [
			DVec3::new(lmin.x, lmin.y, lmin.z),
			DVec3::new(lmax.x, lmin.y, lmin.z),
			DVec3::new(lmin.x, lmax.y, lmin.z),
			DVec3::new(lmax.x, lmax.y, lmin.z),
			DVec3::new(lmin.x, lmin.y, lmax.z),
			DVec3::new(lmax.x, lmin.y, lmax.z),
			DVec3::new(lmin.x, lmax.y, lmax.z),
			DVec3::new(lmax.x, lmax.y, lmax.z),
		];
		let mut wmin = DVec3::splat(f64::INFINITY);
		let mut wmax = DVec3::splat(f64::NEG_INFINITY);
		for c in &corners {
			let w = self.transform.transform_point3(*c);
			wmin = wmin.min(w);
			wmax = wmax.max(w);
		}
		[wmin, wmax]
	}

	/// Local-space AABB (before transform).
	fn local_aabb(&self) -> (DVec3, DVec3) {
		match &self.primitive {
			Primitive::Box { min, max } => (*min, *max),
			Primitive::Sphere { radius } => {
				let r = *radius;
				(DVec3::splat(-r), DVec3::splat(r))
			}
			Primitive::Cylinder { radius, height } => {
				let r = *radius;
				(DVec3::new(-r, -r, 0.0), DVec3::new(r, r, *height))
			}
			Primitive::Cone { r1, r2, height } => {
				let r = r1.max(*r2);
				(DVec3::new(-r, -r, 0.0), DVec3::new(r, r, *height))
			}
			Primitive::Torus { major_radius, minor_radius } => {
				let outer = major_radius + minor_radius;
				(
					DVec3::new(-outer, -outer, -*minor_radius),
					DVec3::new(outer, outer, *minor_radius),
				)
			}
			Primitive::Null => (DVec3::ZERO, DVec3::ZERO),
		}
	}

	/// Build a DAffine3 that places the local +Z axis along `dir` with origin at `origin`.
	fn axis_transform(origin: DVec3, dir: DVec3) -> DAffine3 {
		let z = dir.normalize();
		// Pick a perpendicular vector
		let x = if z.x.abs() < 0.9 {
			DVec3::X.cross(z).normalize()
		} else {
			DVec3::Y.cross(z).normalize()
		};
		let y = z.cross(x);
		DAffine3::from_mat3_translation(DMat3::from_cols(x, y, z), origin)
	}
}

// ==================== SolidTrait ====================

impl SolidTrait for Solid {
	type Face = Face;
	type Edge = Edge;

	// --- Constructors ---

	fn box_from_corners(corner_1: DVec3, corner_2: DVec3) -> Self {
		let min = corner_1.min(corner_2);
		let max = corner_1.max(corner_2);
		Solid::new(Primitive::Box { min, max }, DAffine3::IDENTITY)
	}

	fn sphere(center: DVec3, radius: f64) -> Self {
		Solid::new(
			Primitive::Sphere { radius },
			DAffine3::from_translation(center),
		)
	}

	fn cylinder(p: DVec3, r: f64, dir: DVec3, h: f64) -> Self {
		Solid::new(
			Primitive::Cylinder { radius: r, height: h },
			Self::axis_transform(p, dir),
		)
	}

	fn cone(p: DVec3, dir: DVec3, r1: f64, r2: f64, h: f64) -> Self {
		Solid::new(
			Primitive::Cone { r1, r2, height: h },
			Self::axis_transform(p, dir),
		)
	}

	fn torus(p: DVec3, dir: DVec3, r1: f64, r2: f64) -> Self {
		Solid::new(
			Primitive::Torus { major_radius: r1, minor_radius: r2 },
			Self::axis_transform(p, dir),
		)
	}

	// --- Transforms ---

	fn translate(mut self, translation: DVec3) -> Self {
		self.transform = DAffine3::from_translation(translation) * self.transform;
		self
	}

	fn rotate(mut self, axis_origin: DVec3, axis_direction: DVec3, angle: f64) -> Self {
		let axis = axis_direction.normalize();
		let rotation = DMat3::from_axis_angle(axis, angle);
		// Rotate around axis_origin: T(origin) * R * T(-origin)
		let rot_affine = DAffine3::from_mat3_translation(
			rotation,
			axis_origin - rotation * axis_origin,
		);
		self.transform = rot_affine * self.transform;
		self
	}

	fn scaled(&self, center: DVec3, factor: f64) -> Self {
		let scale = DMat3::from_diagonal(DVec3::splat(factor));
		let scale_affine = DAffine3::from_mat3_translation(
			scale,
			center - scale * center,
		);
		Solid {
			primitive: self.primitive.clone(),
			transform: scale_affine * self.transform,
		}
	}

	fn mirrored(&self, plane_origin: DVec3, plane_normal: DVec3) -> Self {
		let n = plane_normal.normalize();
		// Householder reflection: I - 2*n*nT
		let mirror = DMat3::IDENTITY - 2.0 * DMat3::from_cols(n * n.x, n * n.y, n * n.z);
		let mirror_affine = DAffine3::from_mat3_translation(
			mirror,
			plane_origin - mirror * plane_origin,
		);
		Solid {
			primitive: self.primitive.clone(),
			transform: mirror_affine * self.transform,
		}
	}

	// --- Queries ---

	fn volume(&self) -> f64 {
		self.local_volume() * self.scale_det().abs()
	}

	fn bounding_box(&self) -> [DVec3; 2] {
		self.transformed_aabb()
	}

	fn contains(&self, point: DVec3) -> bool {
		// Transform point to local space
		let inv = self.transform.inverse();
		let local = inv.transform_point3(point);

		match &self.primitive {
			Primitive::Box { min, max } => {
				local.x >= min.x && local.x <= max.x
					&& local.y >= min.y && local.y <= max.y
					&& local.z >= min.z && local.z <= max.z
			}
			Primitive::Sphere { radius } => {
				local.length() <= *radius
			}
			Primitive::Cylinder { radius, height } => {
				let r2d = DVec2::new(local.x, local.y).length();
				r2d <= *radius && local.z >= 0.0 && local.z <= *height
			}
			Primitive::Cone { r1, r2, height } => {
				if local.z < 0.0 || local.z > *height {
					return false;
				}
				let t = local.z / height;
				let r_at_z = r1 * (1.0 - t) + r2 * t;
				DVec2::new(local.x, local.y).length() <= r_at_z
			}
			Primitive::Torus { major_radius, minor_radius } => {
				let r2d = DVec2::new(local.x, local.y).length();
				let d = DVec2::new(r2d - major_radius, local.z).length();
				d <= *minor_radius
			}
			Primitive::Null => false,
		}
	}

	// --- Topology ---

	fn faces(&self) -> Vec<Face> {
		match &self.primitive {
			Primitive::Box { min, max } => {
				let center = (*min + *max) * 0.5;
				let face_defs: [(DVec3, DVec3); 6] = [
					(DVec3::Z,     DVec3::new(center.x, center.y, max.z)),   // +Z top
					(DVec3::NEG_Z, DVec3::new(center.x, center.y, min.z)),   // -Z bottom
					(DVec3::X,     DVec3::new(max.x, center.y, center.z)),   // +X right
					(DVec3::NEG_X, DVec3::new(min.x, center.y, center.z)),   // -X left
					(DVec3::Y,     DVec3::new(center.x, max.y, center.z)),   // +Y back
					(DVec3::NEG_Y, DVec3::new(center.x, min.y, center.z)),   // -Y front
				];
				face_defs
					.iter()
					.map(|&(normal, center)| {
						let world_center = self.transform.transform_point3(center);
						let world_normal = (self.transform.matrix3 * normal).normalize();
						Face { normal: world_normal, center: world_center }
					})
					.collect()
			}
			_ => {
				todo!("faces() not yet implemented for non-Box primitives in pure backend")
			}
		}
	}

	fn edges(&self) -> Vec<Edge> {
		match &self.primitive {
			Primitive::Box { min, max } => {
				let c = [
					DVec3::new(min.x, min.y, min.z), // 0
					DVec3::new(max.x, min.y, min.z), // 1
					DVec3::new(max.x, max.y, min.z), // 2
					DVec3::new(min.x, max.y, min.z), // 3
					DVec3::new(min.x, min.y, max.z), // 4
					DVec3::new(max.x, min.y, max.z), // 5
					DVec3::new(max.x, max.y, max.z), // 6
					DVec3::new(min.x, max.y, max.z), // 7
				];
				let edge_pairs = [
					(0,1),(1,2),(2,3),(3,0), // bottom
					(4,5),(5,6),(6,7),(7,4), // top
					(0,4),(1,5),(2,6),(3,7), // verticals
				];
				edge_pairs
					.iter()
					.map(|&(a, b)| Edge {
						points: vec![
							self.transform.transform_point3(c[a]),
							self.transform.transform_point3(c[b]),
						],
					})
					.collect()
			}
			_ => {
				todo!("edges() not yet implemented for non-Box primitives in pure backend")
			}
		}
	}

	fn clean(&self) -> Result<Self, Error> {
		// Pure backend: analytical primitives don't need cleaning
		Ok(self.clone())
	}

	fn shell_count(&self) -> u32 {
		if self.is_null() { 0 } else { 1 }
	}

	// --- Mesh ---

	fn mesh_with_tolerance(&self, tol: f64) -> Result<Mesh, Error> {
		match &self.primitive {
			Primitive::Box { min, max } => Ok(self.mesh_box(*min, *max)),
			Primitive::Sphere { radius } => Ok(self.mesh_sphere(*radius, tol)),
			Primitive::Cylinder { radius, height } => Ok(self.mesh_cylinder(*radius, *height, tol)),
			Primitive::Cone { r1, r2, height } => Ok(self.mesh_cone(*r1, *r2, *height, tol)),
			Primitive::Torus { major_radius, minor_radius } => Ok(self.mesh_torus(*major_radius, *minor_radius, tol)),
			Primitive::Null => Err(Error::TriangulationFailed),
		}
	}

	// --- Color ---

	#[cfg(feature = "color")]
	fn color_paint(self, _color: Option<crate::common::color::Color>) -> Self {
		// Pure backend: color not supported yet
		self
	}

	#[cfg(feature = "color")]
	fn color(&self) -> Option<crate::common::color::Color> {
		None
	}
}

// ==================== Mesh generation ====================

impl Solid {
	fn transform_mesh(&self, vertices: &mut Vec<DVec3>, normals: &mut Vec<DVec3>) {
		let normal_mat = self.transform.matrix3.inverse().transpose();
		for v in vertices.iter_mut() {
			*v = self.transform.transform_point3(*v);
		}
		for n in normals.iter_mut() {
			*n = (normal_mat * *n).normalize();
		}
	}

	fn mesh_box(&self, min: DVec3, max: DVec3) -> Mesh {
		let c = [
			DVec3::new(min.x, min.y, min.z),
			DVec3::new(max.x, min.y, min.z),
			DVec3::new(max.x, max.y, min.z),
			DVec3::new(min.x, max.y, min.z),
			DVec3::new(min.x, min.y, max.z),
			DVec3::new(max.x, min.y, max.z),
			DVec3::new(max.x, max.y, max.z),
			DVec3::new(min.x, max.y, max.z),
		];

		// 6 faces, each with 4 vertices (shared normals per face) = 24 vertices
		// face order: +Z, -Z, +X, -X, +Y, -Y
		let face_defs: [([usize; 4], DVec3); 6] = [
			([4,5,6,7], DVec3::Z),     // +Z
			([3,2,1,0], DVec3::NEG_Z), // -Z
			([1,2,6,5], DVec3::X),     // +X
			([3,0,4,7], DVec3::NEG_X), // -X
			([2,3,7,6], DVec3::Y),     // +Y
			([0,1,5,4], DVec3::NEG_Y), // -Y
		];

		let mut vertices = Vec::with_capacity(24);
		let mut normals = Vec::with_capacity(24);
		let mut uvs = Vec::with_capacity(24);
		let mut indices = Vec::with_capacity(36);
		let mut face_ids = Vec::with_capacity(12);

		for (fi, (quad, normal)) in face_defs.iter().enumerate() {
			let base = vertices.len();
			for &ci in quad {
				vertices.push(c[ci]);
				normals.push(*normal);
			}
			uvs.extend_from_slice(&[
				DVec2::new(0.0, 0.0),
				DVec2::new(1.0, 0.0),
				DVec2::new(1.0, 1.0),
				DVec2::new(0.0, 1.0),
			]);
			indices.extend_from_slice(&[base, base+1, base+2, base, base+2, base+3]);
			face_ids.push(fi as u64);
			face_ids.push(fi as u64);
		}

		self.transform_mesh(&mut vertices, &mut normals);
		Mesh { vertices, uvs, normals, indices, face_ids, #[cfg(feature = "color")] colormap: std::collections::HashMap::new(), edges: crate::common::mesh::EdgeData::default() }
	}

	fn mesh_sphere(&self, radius: f64, tol: f64) -> Mesh {
		// Subdivisions based on tolerance
		let n_lat = ((PI / tol.sqrt()).ceil() as usize).max(4);
		let n_lon = (n_lat * 2).max(8);

		let mut vertices = Vec::new();
		let mut normals = Vec::new();
		let mut uvs = Vec::new();
		let mut indices = Vec::new();
		let mut face_ids = Vec::new();

		for i in 0..=n_lat {
			let theta = PI * i as f64 / n_lat as f64;
			let sin_t = theta.sin();
			let cos_t = theta.cos();
			for j in 0..=n_lon {
				let phi = 2.0 * PI * j as f64 / n_lon as f64;
				let n = DVec3::new(sin_t * phi.cos(), sin_t * phi.sin(), cos_t);
				vertices.push(n * radius);
				normals.push(n);
				uvs.push(DVec2::new(j as f64 / n_lon as f64, i as f64 / n_lat as f64));
			}
		}

		let row = n_lon + 1;
		for i in 0..n_lat {
			for j in 0..n_lon {
				let a = i * row + j;
				let b = a + 1;
				let c = a + row;
				let d = c + 1;
				if i > 0 {
					indices.extend_from_slice(&[a, c, b]);
					face_ids.push(0);
				}
				if i < n_lat - 1 {
					indices.extend_from_slice(&[b, c, d]);
					face_ids.push(0);
				}
			}
		}

		self.transform_mesh(&mut vertices, &mut normals);
		Mesh { vertices, uvs, normals, indices, face_ids, #[cfg(feature = "color")] colormap: std::collections::HashMap::new(), edges: crate::common::mesh::EdgeData::default() }
	}

	fn mesh_cylinder(&self, radius: f64, height: f64, tol: f64) -> Mesh {
		let n_seg = ((2.0 * PI * radius / tol).ceil() as usize).max(8);

		let mut vertices = Vec::new();
		let mut normals = Vec::new();
		let mut uvs = Vec::new();
		let mut indices = Vec::new();
		let mut face_ids = Vec::new();

		// Side surface: 2 rings
		for i in 0..=n_seg {
			let angle = 2.0 * PI * i as f64 / n_seg as f64;
			let (s, c) = (angle.sin(), angle.cos());
			let n = DVec3::new(c, s, 0.0);
			let u = i as f64 / n_seg as f64;

			vertices.push(DVec3::new(radius * c, radius * s, 0.0));
			normals.push(n);
			uvs.push(DVec2::new(u, 0.0));

			vertices.push(DVec3::new(radius * c, radius * s, height));
			normals.push(n);
			uvs.push(DVec2::new(u, 1.0));
		}
		for i in 0..n_seg {
			let b = i * 2;
			indices.extend_from_slice(&[b, b+2, b+1]);
			face_ids.push(0);
			indices.extend_from_slice(&[b+1, b+2, b+3]);
			face_ids.push(0);
		}

		// Bottom cap (face_id=1)
		let cap_base = vertices.len();
		vertices.push(DVec3::new(0.0, 0.0, 0.0));
		normals.push(DVec3::NEG_Z);
		uvs.push(DVec2::new(0.5, 0.5));
		for i in 0..=n_seg {
			let angle = 2.0 * PI * i as f64 / n_seg as f64;
			let (s, c) = (angle.sin(), angle.cos());
			vertices.push(DVec3::new(radius * c, radius * s, 0.0));
			normals.push(DVec3::NEG_Z);
			uvs.push(DVec2::new(0.5 + 0.5 * c, 0.5 + 0.5 * s));
		}
		for i in 0..n_seg {
			indices.extend_from_slice(&[cap_base, cap_base + 1 + ((i + 1) % (n_seg + 1)), cap_base + 1 + i]);
			face_ids.push(1);
		}

		// Top cap (face_id=2)
		let cap_base = vertices.len();
		vertices.push(DVec3::new(0.0, 0.0, height));
		normals.push(DVec3::Z);
		uvs.push(DVec2::new(0.5, 0.5));
		for i in 0..=n_seg {
			let angle = 2.0 * PI * i as f64 / n_seg as f64;
			let (s, c) = (angle.sin(), angle.cos());
			vertices.push(DVec3::new(radius * c, radius * s, height));
			normals.push(DVec3::Z);
			uvs.push(DVec2::new(0.5 + 0.5 * c, 0.5 + 0.5 * s));
		}
		for i in 0..n_seg {
			indices.extend_from_slice(&[cap_base, cap_base + 1 + i, cap_base + 1 + ((i + 1) % (n_seg + 1))]);
			face_ids.push(2);
		}

		self.transform_mesh(&mut vertices, &mut normals);
		Mesh { vertices, uvs, normals, indices, face_ids, #[cfg(feature = "color")] colormap: std::collections::HashMap::new(), edges: crate::common::mesh::EdgeData::default() }
	}

	fn mesh_cone(&self, r1: f64, r2: f64, height: f64, tol: f64) -> Mesh {
		let max_r = r1.max(r2);
		let n_seg = ((2.0 * PI * max_r / tol).ceil() as usize).max(8);

		let mut vertices = Vec::new();
		let mut normals = Vec::new();
		let mut uvs = Vec::new();
		let mut indices = Vec::new();
		let mut face_ids = Vec::new();

		// Slope for normal computation
		let slope_angle = ((r1 - r2) / height).atan();
		let nz = slope_angle.sin();
		let nr = slope_angle.cos();

		// Side surface
		for i in 0..=n_seg {
			let angle = 2.0 * PI * i as f64 / n_seg as f64;
			let (s, c) = (angle.sin(), angle.cos());
			let n = DVec3::new(nr * c, nr * s, nz).normalize();
			let u = i as f64 / n_seg as f64;

			vertices.push(DVec3::new(r1 * c, r1 * s, 0.0));
			normals.push(n);
			uvs.push(DVec2::new(u, 0.0));

			vertices.push(DVec3::new(r2 * c, r2 * s, height));
			normals.push(n);
			uvs.push(DVec2::new(u, 1.0));
		}
		for i in 0..n_seg {
			let b = i * 2;
			indices.extend_from_slice(&[b, b+2, b+1]);
			face_ids.push(0);
			indices.extend_from_slice(&[b+1, b+2, b+3]);
			face_ids.push(0);
		}

		// Bottom cap (face_id=1)
		if r1 > 1e-12 {
			let cap_base = vertices.len();
			vertices.push(DVec3::new(0.0, 0.0, 0.0));
			normals.push(DVec3::NEG_Z);
			uvs.push(DVec2::new(0.5, 0.5));
			for i in 0..=n_seg {
				let angle = 2.0 * PI * i as f64 / n_seg as f64;
				let (s, c) = (angle.sin(), angle.cos());
				vertices.push(DVec3::new(r1 * c, r1 * s, 0.0));
				normals.push(DVec3::NEG_Z);
				uvs.push(DVec2::new(0.5 + 0.5 * c, 0.5 + 0.5 * s));
			}
			for i in 0..n_seg {
				indices.extend_from_slice(&[cap_base, cap_base + 1 + ((i + 1) % (n_seg + 1)), cap_base + 1 + i]);
				face_ids.push(1);
			}
		}

		// Top cap (face_id=2)
		if r2 > 1e-12 {
			let cap_base = vertices.len();
			vertices.push(DVec3::new(0.0, 0.0, height));
			normals.push(DVec3::Z);
			uvs.push(DVec2::new(0.5, 0.5));
			for i in 0..=n_seg {
				let angle = 2.0 * PI * i as f64 / n_seg as f64;
				let (s, c) = (angle.sin(), angle.cos());
				vertices.push(DVec3::new(r2 * c, r2 * s, height));
				normals.push(DVec3::Z);
				uvs.push(DVec2::new(0.5 + 0.5 * c, 0.5 + 0.5 * s));
			}
			for i in 0..n_seg {
				indices.extend_from_slice(&[cap_base, cap_base + 1 + i, cap_base + 1 + ((i + 1) % (n_seg + 1))]);
				face_ids.push(2);
			}
		}

		self.transform_mesh(&mut vertices, &mut normals);
		Mesh { vertices, uvs, normals, indices, face_ids, #[cfg(feature = "color")] colormap: std::collections::HashMap::new(), edges: crate::common::mesh::EdgeData::default() }
	}

	fn mesh_torus(&self, major_radius: f64, minor_radius: f64, tol: f64) -> Mesh {
		let n_major = ((2.0 * PI * major_radius / tol).ceil() as usize).max(16);
		let n_minor = ((2.0 * PI * minor_radius / tol).ceil() as usize).max(8);

		let mut vertices = Vec::new();
		let mut normals = Vec::new();
		let mut uvs = Vec::new();
		let mut indices = Vec::new();
		let mut face_ids = Vec::new();

		for i in 0..=n_major {
			let theta = 2.0 * PI * i as f64 / n_major as f64;
			let (st, ct) = (theta.sin(), theta.cos());
			for j in 0..=n_minor {
				let phi = 2.0 * PI * j as f64 / n_minor as f64;
				let (sp, cp) = (phi.sin(), phi.cos());

				let x = (major_radius + minor_radius * cp) * ct;
				let y = (major_radius + minor_radius * cp) * st;
				let z = minor_radius * sp;

				let nx = cp * ct;
				let ny = cp * st;
				let nz = sp;

				vertices.push(DVec3::new(x, y, z));
				normals.push(DVec3::new(nx, ny, nz).normalize());
				uvs.push(DVec2::new(
					i as f64 / n_major as f64,
					j as f64 / n_minor as f64,
				));
			}
		}

		let row = n_minor + 1;
		for i in 0..n_major {
			for j in 0..n_minor {
				let a = i * row + j;
				let b = a + 1;
				let c = a + row;
				let d = c + 1;
				indices.extend_from_slice(&[a, c, b]);
				face_ids.push(0);
				indices.extend_from_slice(&[b, c, d]);
				face_ids.push(0);
			}
		}

		self.transform_mesh(&mut vertices, &mut normals);
		Mesh { vertices, uvs, normals, indices, face_ids, #[cfg(feature = "color")] colormap: std::collections::HashMap::new(), edges: crate::common::mesh::EdgeData::default() }
	}
}

#[cfg(test)]
mod test {
	use super::*;

	#[test]
	fn box_volume() {
		let s = Solid::box_from_corners(DVec3::ZERO, DVec3::new(2.0, 3.0, 4.0));
		assert!((s.volume() - 24.0).abs() < 1e-12);
	}

	#[test]
	fn sphere_volume() {
		let s = Solid::sphere(DVec3::ZERO, 1.0);
		let expected = 4.0 / 3.0 * PI;
		assert!((s.volume() - expected).abs() < 1e-12);
	}

	#[test]
	fn cylinder_volume() {
		let s = Solid::cylinder(DVec3::ZERO, 5.0, DVec3::Z, 10.0);
		let expected = PI * 25.0 * 10.0;
		assert!((s.volume() - expected).abs() < 1e-9);
	}

	#[test]
	fn cone_volume() {
		let s = Solid::cone(DVec3::ZERO, DVec3::Z, 3.0, 0.0, 10.0);
		let expected = PI * 10.0 / 3.0 * 9.0;
		assert!((s.volume() - expected).abs() < 1e-9);
	}

	#[test]
	fn torus_volume() {
		let s = Solid::torus(DVec3::ZERO, DVec3::Z, 5.0, 1.0);
		let expected = 2.0 * PI * PI * 5.0 * 1.0;
		assert!((s.volume() - expected).abs() < 1e-9);
	}

	#[test]
	fn box_contains() {
		let s = Solid::box_from_corners(DVec3::ZERO, DVec3::new(10.0, 10.0, 10.0));
		assert!(s.contains(DVec3::new(5.0, 5.0, 5.0)));
		assert!(!s.contains(DVec3::new(15.0, 5.0, 5.0)));
	}

	#[test]
	fn sphere_contains() {
		let s = Solid::sphere(DVec3::new(5.0, 5.0, 5.0), 3.0);
		assert!(s.contains(DVec3::new(5.0, 5.0, 5.0)));
		assert!(!s.contains(DVec3::new(5.0, 5.0, 9.0)));
	}

	#[test]
	fn box_bounding_box() {
		let s = Solid::box_from_corners(DVec3::new(1.0, 2.0, 3.0), DVec3::new(4.0, 6.0, 8.0));
		let [min, max] = s.bounding_box();
		assert!((min - DVec3::new(1.0, 2.0, 3.0)).length() < 1e-10);
		assert!((max - DVec3::new(4.0, 6.0, 8.0)).length() < 1e-10);
	}

	#[test]
	fn translate_preserves_volume() {
		let s = Solid::box_from_corners(DVec3::ZERO, DVec3::splat(10.0));
		let moved = s.translate(DVec3::new(100.0, 200.0, 300.0));
		assert!((moved.volume() - 1000.0).abs() < 1e-9);
	}

	#[test]
	fn scaled_volume() {
		let s = Solid::box_from_corners(DVec3::ZERO, DVec3::splat(10.0));
		let scaled = s.scaled(DVec3::ZERO, 2.0);
		assert!((scaled.volume() - 8000.0).abs() < 1e-6);
	}

	#[test]
	fn box_faces_count() {
		let s = Solid::box_from_corners(DVec3::ZERO, DVec3::splat(10.0));
		assert_eq!(s.faces().len(), 6);
	}

	#[test]
	fn box_edges_count() {
		let s = Solid::box_from_corners(DVec3::ZERO, DVec3::splat(10.0));
		assert_eq!(s.edges().len(), 12);
	}

	#[test]
	fn box_mesh_triangle_count() {
		let s = Solid::box_from_corners(DVec3::ZERO, DVec3::splat(10.0));
		let mesh = s.mesh_with_tolerance(1.0).unwrap();
		assert_eq!(mesh.indices.len() / 3, 12); // 6 faces * 2 triangles
	}

	#[test]
	fn sphere_mesh_has_triangles() {
		let s = Solid::sphere(DVec3::ZERO, 5.0);
		let mesh = s.mesh_with_tolerance(1.0).unwrap();
		assert!(mesh.indices.len() > 0);
		assert!(mesh.vertices.len() > 0);
	}

	#[test]
	fn cylinder_contains() {
		let s = Solid::cylinder(DVec3::ZERO, 5.0, DVec3::Z, 10.0);
		assert!(s.contains(DVec3::new(0.0, 0.0, 5.0)));
		assert!(!s.contains(DVec3::new(6.0, 0.0, 5.0)));
		assert!(!s.contains(DVec3::new(0.0, 0.0, 11.0)));
	}

	#[test]
	fn torus_contains() {
		let s = Solid::torus(DVec3::ZERO, DVec3::Z, 5.0, 1.0);
		assert!(s.contains(DVec3::new(5.0, 0.0, 0.0)));
		assert!(!s.contains(DVec3::new(0.0, 0.0, 0.0)));
	}

	#[test]
	fn rotated_preserves_volume() {
		let s = Solid::box_from_corners(DVec3::ZERO, DVec3::splat(10.0));
		let rotated = s.rotate(DVec3::ZERO, DVec3::Z, PI / 4.0);
		assert!((rotated.volume() - 1000.0).abs() < 1e-6);
	}

	#[test]
	fn mirrored_preserves_volume() {
		let s = Solid::box_from_corners(DVec3::new(1.0, 1.0, 1.0), DVec3::new(2.0, 2.0, 2.0));
		let m = s.mirrored(DVec3::ZERO, DVec3::X);
		assert!((m.volume() - 1.0).abs() < 1e-9);
		let [min, _] = m.bounding_box();
		assert!(min.x < 0.0, "mirrored box should be in -X");
	}
}

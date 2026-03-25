use crate::error::Error;
use crate::ffi;
use crate::iterators::{EdgeIterator, FaceIterator};
use crate::mesh::Mesh;
use crate::solid::Solid;
use glam::{DVec2, DVec3};

// ==================== Color types ====================

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TShapeId(pub u64);

#[cfg(feature = "color")]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rgb {
	pub r: f32,
	pub g: f32,
	pub b: f32,
}

// ==================== Internal helpers ====================

/// Assemble solids into a TopoDS_Compound.
pub(crate) fn to_compound(solids: &[Solid]) -> cxx::UniquePtr<ffi::TopoDS_Shape> {
	let mut compound = ffi::make_empty();
	for s in solids {
		ffi::compound_add(compound.pin_mut(), s.inner());
	}
	compound
}

/// Decompose a compound TopoDS_Shape into Vec<Solid>.
pub(crate) fn decompose(
	compound: &ffi::TopoDS_Shape,
	#[cfg(feature = "color")] colormap: &std::collections::HashMap<TShapeId, Rgb>,
) -> Vec<Solid> {
	let solid_shapes = ffi::decompose_into_solids(compound);
	solid_shapes
		.iter()
		.map(|s| {
			let inner = ffi::shallow_copy(s);
			Solid::new(
				inner,
				#[cfg(feature = "color")]
				colormap.clone(),
			)
		})
		.collect()
}

/// Merge colormaps from all solids.
#[cfg(feature = "color")]
pub(crate) fn merge_all_colormaps(solids: &[Solid]) -> std::collections::HashMap<TShapeId, Rgb> {
	let mut merged = std::collections::HashMap::new();
	for s in solids {
		merged.extend(s.colormap().iter().map(|(&k, &v)| (k, v)));
	}
	merged
}

// ==================== Color helpers ====================

#[cfg(feature = "color")]
pub(crate) fn remap_colormap_by_order(
	old_inner: &ffi::TopoDS_Shape,
	new_inner: &ffi::TopoDS_Shape,
	old_colormap: &std::collections::HashMap<TShapeId, Rgb>,
) -> std::collections::HashMap<TShapeId, Rgb> {
	let mut colormap = std::collections::HashMap::new();
	let old_faces = FaceIterator::new(ffi::explore_faces(old_inner));
	let new_faces = FaceIterator::new(ffi::explore_faces(new_inner));
	for (old_face, new_face) in old_faces.zip(new_faces) {
		if let Some(&color) = old_colormap.get(&old_face.tshape_id()) {
			colormap.insert(new_face.tshape_id(), color);
		}
	}
	colormap
}

#[cfg(feature = "color")]
fn merge_colormaps(
	from_a: &[u64],
	from_b: &[u64],
	colormap_a: &std::collections::HashMap<TShapeId, Rgb>,
	colormap_b: &std::collections::HashMap<TShapeId, Rgb>,
) -> std::collections::HashMap<TShapeId, Rgb> {
	let mut result = std::collections::HashMap::new();
	for pair in from_a.chunks(2) {
		if let Some(&color) = colormap_a.get(&TShapeId(pair[1])) {
			result.insert(TShapeId(pair[0]), color);
		}
	}
	for pair in from_b.chunks(2) {
		if let Some(&color) = colormap_b.get(&TShapeId(pair[1])) {
			result.insert(TShapeId(pair[0]), color);
		}
	}
	result
}

// ==================== BooleanShape ====================

/// Result of a boolean operation.
pub struct BooleanShape {
	pub solids: Vec<Solid>,
	from_a: Vec<u64>,
	from_b: Vec<u64>,
}

impl BooleanShape {
	/// Returns `true` if `face` originated from the `other` (tool) operand.
	pub fn is_tool_face(&self, face: &crate::face::Face) -> bool {
		self.from_b.contains(&face.tshape_id().0)
	}

	/// Returns `true` if `face` originated from `self` (the base shape operand).
	pub fn is_shape_face(&self, face: &crate::face::Face) -> bool {
		self.from_a.contains(&face.tshape_id().0)
	}
}

impl From<BooleanShape> for Vec<Solid> {
	fn from(r: BooleanShape) -> Vec<Solid> {
		r.solids
	}
}

// ==================== Shape trait ====================

/// Trait for operations on `[Solid]`.
///
/// Import this trait to use methods on `Vec<Solid>` / `&[Solid]`:
/// ```
/// use chijin::Shape;
/// ```
pub trait Shape {
	// --- Boolean ---
	fn union(&self, other: &[Solid]) -> Result<BooleanShape, Error>;
	fn subtract(&self, other: &[Solid]) -> Result<BooleanShape, Error>;
	fn intersect(&self, other: &[Solid]) -> Result<BooleanShape, Error>;

	// --- Transforms ---
	fn translated(&self, translation: DVec3) -> Vec<Solid>;
	fn rotated(&self, axis_origin: DVec3, axis_direction: DVec3, angle: f64) -> Vec<Solid>;
	fn scaled(&self, center: DVec3, factor: f64) -> Vec<Solid>;
	fn clean(&self) -> Result<Vec<Solid>, Error>;

	// --- Aggregate queries ---
	fn volume(&self) -> f64;
	fn contains(&self, point: DVec3) -> bool;
	fn is_null(&self) -> bool;
	fn shell_count(&self) -> u32;

	// --- Topology / rendering ---
	fn faces(&self) -> FaceIterator;
	fn edges(&self) -> EdgeIterator;
	fn mesh_with_tolerance(&self, tol: f64) -> Result<Mesh, Error>;
	fn to_svg(&self, direction: DVec3, tolerance: f64) -> Result<String, Error>;

	// --- Color ---
	#[cfg(feature = "color")]
	fn paint(&mut self, color: Rgb);
}

// ==================== impl Shape for [Solid] ====================

impl Shape for [Solid] {
	// --- Boolean ---

	fn union(&self, other: &[Solid]) -> Result<BooleanShape, Error> {
		let c_self = to_compound(self);
		let c_other = to_compound(other);
		let r = ffi::boolean_fuse(&c_self, &c_other);
		if r.is_null() {
			return Err(Error::BooleanOperationFailed);
		}
		build_boolean_result(
			r,
			self,
			other,
		)
	}

	fn subtract(&self, other: &[Solid]) -> Result<BooleanShape, Error> {
		let c_self = to_compound(self);
		let c_other = to_compound(other);
		let r = ffi::boolean_cut(&c_self, &c_other);
		if r.is_null() {
			return Err(Error::BooleanOperationFailed);
		}
		build_boolean_result(
			r,
			self,
			other,
		)
	}

	fn intersect(&self, other: &[Solid]) -> Result<BooleanShape, Error> {
		let c_self = to_compound(self);
		let c_other = to_compound(other);
		let r = ffi::boolean_common(&c_self, &c_other);
		if r.is_null() {
			return Err(Error::BooleanOperationFailed);
		}
		build_boolean_result(
			r,
			self,
			other,
		)
	}

	// --- Transforms ---

	fn translated(&self, translation: DVec3) -> Vec<Solid> {
		self.iter().map(|s| s.translated(translation)).collect()
	}

	fn rotated(&self, axis_origin: DVec3, axis_direction: DVec3, angle: f64) -> Vec<Solid> {
		self.iter().map(|s| s.rotated(axis_origin, axis_direction, angle)).collect()
	}

	fn scaled(&self, center: DVec3, factor: f64) -> Vec<Solid> {
		self.iter().map(|s| s.scaled(center, factor)).collect()
	}

	fn clean(&self) -> Result<Vec<Solid>, Error> {
		self.iter().map(|s| s.clean()).collect()
	}

	// --- Aggregate queries ---

	fn volume(&self) -> f64 {
		self.iter().map(|s| s.volume()).sum()
	}

	fn contains(&self, point: DVec3) -> bool {
		self.iter().any(|s| s.contains(point))
	}

	fn is_null(&self) -> bool {
		self.is_empty() || self.iter().all(|s| s.is_null())
	}

	fn shell_count(&self) -> u32 {
		self.iter().map(|s| s.shell_count()).sum()
	}

	// --- Topology / rendering ---

	fn faces(&self) -> FaceIterator {
		let compound = to_compound(self);
		FaceIterator::new(ffi::explore_faces(&compound))
	}

	fn edges(&self) -> EdgeIterator {
		let compound = to_compound(self);
		EdgeIterator::new(ffi::explore_edges(&compound))
	}

	fn mesh_with_tolerance(&self, tol: f64) -> Result<Mesh, Error> {
		let compound = to_compound(self);
		let data = ffi::mesh_shape(&compound, tol);
		if !data.success {
			return Err(Error::TriangulationFailed);
		}
		let vertex_count = data.vertices.len() / 3;
		let vertices: Vec<DVec3> = (0..vertex_count)
			.map(|i| DVec3::new(data.vertices[i * 3], data.vertices[i * 3 + 1], data.vertices[i * 3 + 2]))
			.collect();
		let uvs: Vec<DVec2> = (0..vertex_count)
			.map(|i| DVec2::new(data.uvs[i * 2], data.uvs[i * 2 + 1]))
			.collect();
		let normals: Vec<DVec3> = (0..vertex_count)
			.map(|i| DVec3::new(data.normals[i * 3], data.normals[i * 3 + 1], data.normals[i * 3 + 2]))
			.collect();
		let indices: Vec<usize> = data.indices.iter().map(|&i| i as usize).collect();
		let face_ids = data.face_tshape_ids;
		Ok(Mesh { vertices, uvs, normals, indices, face_ids })
	}

	fn to_svg(&self, direction: DVec3, tolerance: f64) -> Result<String, Error> {
		let cleaned: Vec<Solid> = self.clean()?;
		let compound = to_compound(&cleaned);

		let edge_data = ffi::project_shape_hlr(
			&compound,
			direction.x, direction.y, direction.z,
			tolerance,
		);
		if !edge_data.success {
			return Err(Error::SvgExportFailed);
		}

		let mesh = cleaned.mesh_with_tolerance(tolerance)?;
		let face_triangles = project_and_sort_triangles(
			&mesh,
			direction,
			#[cfg(feature = "color")]
			&merge_all_colormaps(&cleaned),
		);

		Ok(build_svg(&edge_data, &face_triangles))
	}

	// --- Color ---

	#[cfg(feature = "color")]
	fn paint(&mut self, color: Rgb) {
		for s in self.iter_mut() {
			s.paint(color);
		}
	}
}

// ==================== Boolean helper ====================

fn build_boolean_result(
	r: cxx::UniquePtr<ffi::BooleanShape>,
	self_solids: &[Solid],
	other_solids: &[Solid],
) -> Result<BooleanShape, Error> {
	let from_a = ffi::boolean_shape_from_a(&r);
	let from_b = ffi::boolean_shape_from_b(&r);
	let inner = ffi::boolean_shape_shape(&r);

	#[cfg(feature = "color")]
	let colormap = {
		let colormap_a = merge_all_colormaps(self_solids);
		let colormap_b = merge_all_colormaps(other_solids);
		merge_colormaps(&from_a, &from_b, &colormap_a, &colormap_b)
	};
	#[cfg(not(feature = "color"))]
	let _ = (self_solids, other_solids);

	let solids = decompose(
		&inner,
		#[cfg(feature = "color")]
		&colormap,
	);

	Ok(BooleanShape { solids, from_a, from_b })
}

// ==================== SVG helpers ====================

struct SvgTriangle {
	pts: [(f64, f64); 3],
	depth: f64,
	fill: String,
}

fn occt_ax2_basis(dir: DVec3) -> (DVec3, DVec3) {
	let (a, b, c) = (dir.x, dir.y, dir.z);
	let (a_abs, b_abs, c_abs) = (a.abs(), b.abs(), c.abs());

	let perp = if b_abs <= a_abs && b_abs <= c_abs {
		if a_abs > c_abs { DVec3::new(-c, 0.0, a) } else { DVec3::new(c, 0.0, -a) }
	} else if a_abs <= b_abs && a_abs <= c_abs {
		if b_abs > c_abs { DVec3::new(0.0, -c, b) } else { DVec3::new(0.0, c, -b) }
	} else {
		if a_abs > b_abs { DVec3::new(-b, a, 0.0) } else { DVec3::new(b, -a, 0.0) }
	};

	let x_dir = perp.normalize();
	let y_dir = dir.cross(x_dir);
	(x_dir, y_dir)
}

fn project_and_sort_triangles(
	mesh: &Mesh,
	direction: DVec3,
	#[cfg(feature = "color")] colormap: &std::collections::HashMap<TShapeId, Rgb>,
) -> Vec<SvgTriangle> {
	let dir = direction.normalize();
	let (u, v) = occt_ax2_basis(dir);

	let tri_count = mesh.indices.len() / 3;
	let mut triangles = Vec::with_capacity(tri_count);

	for ti in 0..tri_count {
		let i0 = mesh.indices[ti * 3];
		let i1 = mesh.indices[ti * 3 + 1];
		let i2 = mesh.indices[ti * 3 + 2];

		let v0 = mesh.vertices[i0];
		let v1 = mesh.vertices[i1];
		let v2 = mesh.vertices[i2];

		let avg_normal = (mesh.normals[i0] + mesh.normals[i1] + mesh.normals[i2]) / 3.0;
		if avg_normal.dot(dir) < 0.0 {
			continue;
		}

		let p0 = (v0.dot(u), v0.dot(v));
		let p1 = (v1.dot(u), v1.dot(v));
		let p2 = (v2.dot(u), v2.dot(v));

		let depth = (v0.dot(dir) + v1.dot(dir) + v2.dot(dir)) / 3.0;

		#[cfg(feature = "color")]
		let fill = {
			let face_id = TShapeId(mesh.face_ids[ti]);
			if let Some(c) = colormap.get(&face_id) {
				format!("rgb({},{},{})", (c.r * 255.0) as u8, (c.g * 255.0) as u8, (c.b * 255.0) as u8)
			} else {
				"#ddd".to_string()
			}
		};
		#[cfg(not(feature = "color"))]
		let fill = "#ddd".to_string();

		triangles.push(SvgTriangle { pts: [p0, p1, p2], depth, fill });
	}

	triangles.sort_by(|a, b| a.depth.partial_cmp(&b.depth).unwrap_or(std::cmp::Ordering::Equal));
	triangles
}

fn polylines_to_svg(svg: &mut String, coords: &[f64], counts: &[u32], stroke: &str, dash: &str) {
	let mut offset = 0usize;
	for &count in counts {
		let n = count as usize;
		svg.push_str("<polyline points=\"");
		for i in 0..n {
			let x = coords[(offset + i) * 2];
			let y = -coords[(offset + i) * 2 + 1];
			if i > 0 { svg.push(' '); }
			svg.push_str(&format!("{x:.4},{y:.4}"));
		}
		svg.push_str("\" fill=\"none\" stroke=\"");
		svg.push_str(stroke);
		svg.push('"');
		if !dash.is_empty() {
			svg.push_str(" stroke-dasharray=\"");
			svg.push_str(dash);
			svg.push('"');
		}
		svg.push_str("/>\n");
		offset += n;
	}
}

fn build_svg(edge_data: &ffi::SvgEdgeData, triangles: &[SvgTriangle]) -> String {
	let mut min_x = edge_data.min_x;
	let mut min_y = edge_data.min_y;
	let mut max_x = edge_data.max_x;
	let mut max_y = edge_data.max_y;
	for tri in triangles {
		for &(x, y) in &tri.pts {
			if x < min_x { min_x = x; }
			if x > max_x { max_x = x; }
			if y < min_y { min_y = y; }
			if y > max_y { max_y = y; }
		}
	}

	let margin_frac = 0.05;
	let w = max_x - min_x;
	let h = max_y - min_y;
	let margin = if w > h { w } else { h } * margin_frac;
	let vx = min_x - margin;
	let vy = -(max_y + margin);
	let vw = w + margin * 2.0;
	let vh = h + margin * 2.0;
	let sw = (if w > h { w } else { h }) * 0.003;
	let dash_len = sw * 3.0;

	let mut svg = String::with_capacity(4096 + triangles.len() * 120);
	svg.push_str(&format!(
		"<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"{vx:.4} {vy:.4} {vw:.4} {vh:.4}\" \
		 stroke-width=\"{sw:.4}\">\n"
	));

	for tri in triangles {
		let [(x0, y0), (x1, y1), (x2, y2)] = tri.pts;
		let y0 = -y0;
		let y1 = -y1;
		let y2 = -y2;
		svg.push_str(&format!(
			"<polygon points=\"{x0:.4},{y0:.4} {x1:.4},{y1:.4} {x2:.4},{y2:.4}\" \
			 fill=\"{}\" stroke=\"none\"/>\n",
			tri.fill
		));
	}

	polylines_to_svg(&mut svg, &edge_data.visible_coords, &edge_data.visible_counts, "black", "");
	polylines_to_svg(
		&mut svg,
		&edge_data.hidden_coords,
		&edge_data.hidden_counts,
		"#999",
		&format!("{dash_len:.4},{dash_len:.4}"),
	);

	svg.push_str("</svg>\n");
	svg
}

#[cfg(feature = "color")]
use super::color::Color;
use glam::{DVec2, DVec3};
use std::collections::HashMap;

/// 3D edge polylines for SVG rendering.
///
/// Stores topological edges as 3D polylines. Visibility classification
/// (visible vs hidden) is computed by [`Mesh::to_svg`] when hidden line
/// rendering is enabled.
#[derive(Debug, Clone, Default)]
pub struct EdgeData {
	/// 3D polylines representing topological edges.
	pub polylines: Vec<Vec<DVec3>>,
}

/// A triangle mesh produced by meshing a solid shape.
///
/// All vectors have the same length: one entry per vertex.
/// `indices` contains triangle indices (groups of 3).
#[derive(Debug, Clone)]
pub struct Mesh {
	/// Vertex positions in 3D space.
	pub vertices: Vec<DVec3>,
	/// UV coordinates, normalized to [0, 1] per face.
	pub uvs: Vec<DVec2>,
	/// Vertex normals.
	pub normals: Vec<DVec3>,
	/// Triangle indices (groups of 3, referencing into `vertices`).
	pub indices: Vec<usize>,
	/// Per-triangle face ID. Length equals `indices.len() / 3`.
	pub face_ids: Vec<u64>,
	/// Per-face color map (face_id → Color).
	#[cfg(feature = "color")]
	pub colormap: HashMap<u64, Color>,
	/// Topological edge polylines for SVG rendering.
	pub edges: EdgeData,
}

// ==================== STL ====================

impl Mesh {
	/// Write this mesh as binary STL to a writer.
	/// このメッシュをバイナリ STL 形式で書き出す。
	pub fn write_stl<W: std::io::Write>(&self, writer: &mut W) -> Result<(), super::error::Error> {
		let tri_count = self.indices.len() / 3;
		// 80-byte header
		writer.write_all(&[0u8; 80]).map_err(|_| super::error::Error::StlWriteFailed)?;
		// Triangle count (u32 LE)
		writer.write_all(&(tri_count as u32).to_le_bytes()).map_err(|_| super::error::Error::StlWriteFailed)?;
		for ti in 0..tri_count {
			let i0 = self.indices[ti * 3];
			let i1 = self.indices[ti * 3 + 1];
			let i2 = self.indices[ti * 3 + 2];
			let v0 = self.vertices[i0];
			let v1 = self.vertices[i1];
			let v2 = self.vertices[i2];
			// Face normal from cross product / 外積から面法線を計算
			let n = (v1 - v0).cross(v2 - v0).normalize_or_zero();
			// Normal (3 x f32 LE)
			for c in [n.x, n.y, n.z] { writer.write_all(&(c as f32).to_le_bytes()).map_err(|_| super::error::Error::StlWriteFailed)?; }
			// Vertices (3 x 3 x f32 LE)
			for v in [v0, v1, v2] {
				for c in [v.x, v.y, v.z] { writer.write_all(&(c as f32).to_le_bytes()).map_err(|_| super::error::Error::StlWriteFailed)?; }
			}
			// Attribute byte count — RGB555 color (SolidView/MeshLab convention)
			#[cfg(feature = "color")]
			let attr = {
				let face_id = self.face_ids[ti];
				if let Some(c) = self.colormap.get(&face_id) {
					let r = (c.r * 31.0) as u16;
					let g = (c.g * 31.0) as u16;
					let b = (c.b * 31.0) as u16;
					0x8000 | r | (g << 5) | (b << 10)
				} else {
					0u16
				}
			};
			#[cfg(not(feature = "color"))]
			let attr = 0u16;
			writer.write_all(&attr.to_le_bytes()).map_err(|_| super::error::Error::StlWriteFailed)?;
		}
		Ok(())
	}
}

// ==================== SVG ====================

impl Mesh {
	/// Render this mesh as an SVG string.
	///
	/// `direction` is the viewing direction (the direction the camera looks from;
	/// points with higher `dot(direction)` are closer to the camera).
	///
	/// `hidden_lines` controls whether occluded edges are rendered as faint dashed
	/// lines. Set to `false` for cleaner output on dense models (e.g. helical
	/// sweeps) where hidden lines dominate the image.
	///
	/// `shading` enables Lambertian shading with head-on light
	/// (light direction == `direction`). Front-facing triangles get
	/// `shade = 0.5 + 0.5 * (normal · dir)`, so glancing faces darken to
	/// half intensity. Turn this on for curved/organic shapes where flat
	/// fill makes the 3D form hard to read; leave it off for clean flat
	/// rendering matching earlier cadrum output.
	///
	/// The method:
	/// 1. Projects triangles onto the plane perpendicular to `direction`
	/// 2. Detects silhouette edges from mesh adjacency
	/// 3. Classifies edges as visible or hidden (only when `hidden_lines`)
	/// 4. Renders colored triangles, visible edges (black), and optionally hidden edges
	pub fn write_svg<W: std::io::Write>(&self, direction: DVec3, hidden_lines: bool, shading: bool, writer: &mut W) -> Result<(), super::error::Error> {
		writer.write_all(self.to_svg(direction, hidden_lines, shading).as_bytes()).map_err(|_| super::error::Error::SvgExportFailed)
	}

	pub fn to_svg(&self, direction: DVec3, hidden_lines: bool, shading: bool) -> String {
		let dir = direction.normalize();
		let (u, v) = projection_basis(dir);

		// 1. Project and sort triangles for rendering
		let face_triangles = project_and_sort_triangles(self, dir, u, v, shading);

		// 2. Detect silhouette edges from mesh adjacency
		let silhouette_edges = detect_silhouette_edges(self, dir);

		// 3. Combine topological edges + silhouette edges
		let all_edges: Vec<&Vec<DVec3>> = self.edges.polylines.iter().chain(silhouette_edges.iter()).collect();

		// 4. Classify edges. When hidden lines are disabled we still need to
		//    drop occluded segments from the visible set, so build occlusion
		//    data and reuse the same classifier — only the hidden output is
		//    discarded.
		let occlusion_tris = build_occlusion_data(self, dir, u, v);
		let (visible, hidden) = classify_edges(&all_edges, &occlusion_tris, dir, u, v);
		let hidden = if hidden_lines { hidden } else { Vec::new() };

		// 5. Build SVG
		build_svg(&face_triangles, &visible, &hidden)
	}
}

// ==================== SVG internals ====================

struct SvgTriangle {
	pts: [(f64, f64); 3],
	depth: f64,
	fill: String,
}

/// Projected front-facing triangle for occlusion testing.
struct OcclusionTri {
	pts: [(f64, f64); 3],
	depths: [f64; 3],
}

/// Compute an orthonormal basis (x_dir, y_dir) for the projection plane
/// perpendicular to `dir`. Matches OpenCASCADE's gp_Ax2 convention.
fn projection_basis(dir: DVec3) -> (DVec3, DVec3) {
	let (a, b, c) = (dir.x, dir.y, dir.z);
	let (a_abs, b_abs, c_abs) = (a.abs(), b.abs(), c.abs());

	let perp = if b_abs <= a_abs && b_abs <= c_abs {
		if a_abs > c_abs {
			DVec3::new(-c, 0.0, a)
		} else {
			DVec3::new(c, 0.0, -a)
		}
	} else if a_abs <= b_abs && a_abs <= c_abs {
		if b_abs > c_abs {
			DVec3::new(0.0, -c, b)
		} else {
			DVec3::new(0.0, c, -b)
		}
	} else {
		if a_abs > b_abs {
			DVec3::new(-b, a, 0.0)
		} else {
			DVec3::new(b, -a, 0.0)
		}
	};

	let x_dir = perp.normalize();
	let y_dir = dir.cross(x_dir);
	(x_dir, y_dir)
}

fn project_and_sort_triangles(mesh: &Mesh, dir: DVec3, u: DVec3, v: DVec3, shading: bool) -> Vec<SvgTriangle> {
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

		// Lambertian shading with head-on light (light direction == view direction).
		// Front-facing triangles get `normal · dir ∈ (0, 1]`; normalize to handle
		// the averaged normal's non-unit length. Shade maps [0, 1] → [0.5, 1.0]
		// so glancing faces darken to half-intensity (not black) — enough to
		// read the 3D shape without swallowing the silhouette into the stroke.
		// When `shading` is false, every triangle gets shade=1.0 → flat fill,
		// matching the pre-shading output (`#ddd` for no-color path).
		let shade = if shading {
			let dot = avg_normal.normalize_or_zero().dot(dir).clamp(0.0, 1.0);
			0.5 + 0.5 * dot
		} else {
			1.0
		};

		let gray = 0xdd as f64 / 255.0;
		#[cfg(feature = "color")]
		let (base_r, base_g, base_b) = {
			let face_id = mesh.face_ids[ti];
			if let Some(c) = mesh.colormap.get(&face_id) {
				(c.r as f64, c.g as f64, c.b as f64)
			} else {
				(gray, gray, gray)
			}
		};
		#[cfg(not(feature = "color"))]
		let (base_r, base_g, base_b) = (gray, gray, gray);

		// Emit fill as `#rrggbb` hex (7 chars) — shorter than `rgb(R,G,B)`.
		// When `shading` is off, `shade == 1.0` so the formula collapses to
		// the base color (every front-facing triangle shares the same fill
		// and the SVG stays compact).
		let fill = format!(
			"#{:02x}{:02x}{:02x}",
			(base_r * shade * 255.0) as u8,
			(base_g * shade * 255.0) as u8,
			(base_b * shade * 255.0) as u8,
		);

		triangles.push(SvgTriangle { pts: [p0, p1, p2], depth, fill });
	}

	triangles.sort_by(|a, b| a.depth.partial_cmp(&b.depth).unwrap_or(std::cmp::Ordering::Equal));
	triangles
}

/// Build projected front-facing triangles for occlusion testing.
fn build_occlusion_data(mesh: &Mesh, dir: DVec3, u: DVec3, v: DVec3) -> Vec<OcclusionTri> {
	let tri_count = mesh.indices.len() / 3;
	let mut tris = Vec::with_capacity(tri_count / 2);

	for ti in 0..tri_count {
		let i0 = mesh.indices[ti * 3];
		let i1 = mesh.indices[ti * 3 + 1];
		let i2 = mesh.indices[ti * 3 + 2];

		let v0 = mesh.vertices[i0];
		let v1 = mesh.vertices[i1];
		let v2 = mesh.vertices[i2];

		let avg_normal = (mesh.normals[i0] + mesh.normals[i1] + mesh.normals[i2]) / 3.0;
		if avg_normal.dot(dir) <= 0.0 {
			continue;
		}

		tris.push(OcclusionTri { pts: [(v0.dot(u), v0.dot(v)), (v1.dot(u), v1.dot(v)), (v2.dot(u), v2.dot(v))], depths: [v0.dot(dir), v1.dot(dir), v2.dot(dir)] });
	}
	tris
}

/// Detect silhouette edges from mesh triangle adjacency.
///
/// A silhouette edge is one where:
/// - One adjacent triangle faces the camera, the other faces away (contour edge)
/// - Only one adjacent triangle exists (boundary edge) and it faces the camera
fn detect_silhouette_edges(mesh: &Mesh, dir: DVec3) -> Vec<Vec<DVec3>> {
	let tri_count = mesh.indices.len() / 3;

	// Build edge → triangle adjacency map
	let mut edge_tris: HashMap<(usize, usize), Vec<usize>> = HashMap::new();
	for ti in 0..tri_count {
		let i0 = mesh.indices[ti * 3];
		let i1 = mesh.indices[ti * 3 + 1];
		let i2 = mesh.indices[ti * 3 + 2];
		for &(a, b) in &[(i0, i1), (i1, i2), (i2, i0)] {
			let key = if a < b { (a, b) } else { (b, a) };
			edge_tris.entry(key).or_default().push(ti);
		}
	}

	let mut silhouettes = Vec::new();
	for (&(a, b), tris) in &edge_tris {
		let is_silhouette = if tris.len() == 1 {
			// Boundary edge: silhouette if the single adjacent face is front-facing
			tri_facing(mesh, tris[0], dir)
		} else if tris.len() == 2 {
			// Contour edge: one front-facing, one back-facing
			tri_facing(mesh, tris[0], dir) != tri_facing(mesh, tris[1], dir)
		} else {
			false
		};
		if is_silhouette {
			silhouettes.push(vec![mesh.vertices[a], mesh.vertices[b]]);
		}
	}
	silhouettes
}

/// Returns true if triangle `ti` is front-facing relative to `dir`.
fn tri_facing(mesh: &Mesh, ti: usize, dir: DVec3) -> bool {
	let i0 = mesh.indices[ti * 3];
	let i1 = mesh.indices[ti * 3 + 1];
	let i2 = mesh.indices[ti * 3 + 2];
	let avg_normal = (mesh.normals[i0] + mesh.normals[i1] + mesh.normals[i2]) / 3.0;
	avg_normal.dot(dir) > 0.0
}

/// Classify edge segments as visible or hidden based on triangle occlusion.
///
/// Returns (visible_polylines, hidden_polylines) as 2D projected coordinates.
fn classify_edges(edges: &[&Vec<DVec3>], occlusion_tris: &[OcclusionTri], dir: DVec3, u: DVec3, v: DVec3) -> (Vec<Vec<(f64, f64)>>, Vec<Vec<(f64, f64)>>) {
	let mut visible_polylines = Vec::new();
	let mut hidden_polylines = Vec::new();

	for edge in edges {
		if edge.len() < 2 {
			continue;
		}

		let mut vis_line: Vec<(f64, f64)> = Vec::new();
		let mut hid_line: Vec<(f64, f64)> = Vec::new();

		for window in edge.windows(2) {
			let a3d = window[0];
			let b3d = window[1];
			let mid = (a3d + b3d) * 0.5;
			let mid_2d = (mid.dot(u), mid.dot(v));
			let mid_depth = mid.dot(dir);

			let a_2d = (a3d.dot(u), a3d.dot(v));
			let b_2d = (b3d.dot(u), b3d.dot(v));

			let hidden = is_point_occluded(mid_2d, mid_depth, occlusion_tris);

			if hidden {
				// Flush visible line if any
				if vis_line.len() >= 2 {
					visible_polylines.push(std::mem::take(&mut vis_line));
				} else {
					vis_line.clear();
				}
				// Extend or start hidden line
				if hid_line.is_empty() {
					hid_line.push(a_2d);
				}
				hid_line.push(b_2d);
			} else {
				// Flush hidden line if any
				if hid_line.len() >= 2 {
					hidden_polylines.push(std::mem::take(&mut hid_line));
				} else {
					hid_line.clear();
				}
				// Extend or start visible line
				if vis_line.is_empty() {
					vis_line.push(a_2d);
				}
				vis_line.push(b_2d);
			}
		}

		if vis_line.len() >= 2 {
			visible_polylines.push(vis_line);
		}
		if hid_line.len() >= 2 {
			hidden_polylines.push(hid_line);
		}
	}

	(visible_polylines, hidden_polylines)
}

/// Check if a 2D point at a given depth is occluded by any front-facing triangle.
fn is_point_occluded(point_2d: (f64, f64), point_depth: f64, tris: &[OcclusionTri]) -> bool {
	// Tolerance for self-occlusion: edge lies on the surface, so its depth
	// is approximately equal to the adjacent face's depth.
	let eps = 1e-4;

	for tri in tris {
		if let Some((w0, w1, w2)) = barycentric_2d(point_2d, tri.pts) {
			let tri_depth = w0 * tri.depths[0] + w1 * tri.depths[1] + w2 * tri.depths[2];
			if tri_depth > point_depth + eps {
				return true; // triangle is closer to camera than the edge
			}
		}
	}
	false
}

/// Compute barycentric coordinates of point `p` in triangle `t` (2D).
/// Returns Some((w0, w1, w2)) if the point is inside the triangle.
fn barycentric_2d(p: (f64, f64), t: [(f64, f64); 3]) -> Option<(f64, f64, f64)> {
	let (px, py) = p;
	let (x0, y0) = t[0];
	let (x1, y1) = t[1];
	let (x2, y2) = t[2];

	let denom = (y1 - y2) * (x0 - x2) + (x2 - x1) * (y0 - y2);
	if denom.abs() < 1e-12 {
		return None; // degenerate triangle
	}

	let w0 = ((y1 - y2) * (px - x2) + (x2 - x1) * (py - y2)) / denom;
	let w1 = ((y2 - y0) * (px - x2) + (x0 - x2) * (py - y2)) / denom;
	let w2 = 1.0 - w0 - w1;

	// Small negative tolerance for edges
	if w0 >= -1e-8 && w1 >= -1e-8 && w2 >= -1e-8 {
		Some((w0, w1, w2))
	} else {
		None
	}
}

// ==================== SVG generation ====================

fn polylines_to_svg(svg: &mut String, polylines: &[Vec<(f64, f64)>], stroke: &str, dash: &str, width: Option<f64>) {
	for line in polylines {
		svg.push_str("<polyline points=\"");
		for (i, &(x, y)) in line.iter().enumerate() {
			let y = -y;
			if i > 0 {
				svg.push(' ');
			}
			svg.push_str(&format!("{x:.4},{y:.4}"));
		}
		svg.push_str("\" fill=\"none\" stroke=\"");
		svg.push_str(stroke);
		svg.push('"');
		if let Some(w) = width {
			svg.push_str(&format!(" stroke-width=\"{w:.4}\""));
		}
		if !dash.is_empty() {
			svg.push_str(" stroke-dasharray=\"");
			svg.push_str(dash);
			svg.push('"');
		}
		svg.push_str("/>\n");
	}
}

fn build_svg(triangles: &[SvgTriangle], visible_lines: &[Vec<(f64, f64)>], hidden_lines: &[Vec<(f64, f64)>]) -> String {
	// Compute bounding box from triangles and edges
	let mut min_x = f64::INFINITY;
	let mut min_y = f64::INFINITY;
	let mut max_x = f64::NEG_INFINITY;
	let mut max_y = f64::NEG_INFINITY;

	for tri in triangles {
		for &(x, y) in &tri.pts {
			if x < min_x {
				min_x = x;
			}
			if x > max_x {
				max_x = x;
			}
			if y < min_y {
				min_y = y;
			}
			if y > max_y {
				max_y = y;
			}
		}
	}

	for lines in [visible_lines, hidden_lines] {
		for line in lines {
			for &(x, y) in line {
				if x < min_x {
					min_x = x;
				}
				if x > max_x {
					max_x = x;
				}
				if y < min_y {
					min_y = y;
				}
				if y > max_y {
					max_y = y;
				}
			}
		}
	}

	// Handle empty case
	if min_x > max_x {
		min_x = 0.0;
		max_x = 1.0;
		min_y = 0.0;
		max_y = 1.0;
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
	// Hidden lines: thinner stroke and longer dashes to reduce visual clutter
	// on dense models (e.g. helical sweeps).
	let hidden_sw = sw * 0.6;
	let dash_len = sw * 5.0;
	let dash_gap = sw * 4.0;

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

	polylines_to_svg(&mut svg, visible_lines, "black", "", None);
	polylines_to_svg(&mut svg, hidden_lines, "#bbb", &format!("{dash_len:.4},{dash_gap:.4}"), Some(hidden_sw));

	svg.push_str("</svg>\n");
	svg
}

use super::stream::{rust_reader_read, rust_writer_flush, rust_writer_write};
use super::stream::{RustReader, RustWriter};

#[cxx::bridge(namespace = "cadrum")]
mod ffi_bridge {
	// Shared struct for mesh data returned from C++
	struct MeshData {
		vertices: Vec<f64>, // flat xyz
		uvs: Vec<f64>,      // flat uv
		normals: Vec<f64>,  // flat xyz
		indices: Vec<u32>,
		face_tshape_ids: Vec<u64>, // per-triangle TShape* address
		success: bool,
	}

	// Shared struct for approximation points
	struct ApproxPoints {
		coords: Vec<f64>, // flat xyz
		count: u32,
	}

	// Expose Rust stream types to C++ for streambuf callbacks
	extern "Rust" {
		type RustReader;
		type RustWriter;

		fn rust_reader_read(reader: &mut RustReader, buf: &mut [u8]) -> usize;
		fn rust_writer_write(writer: &mut RustWriter, buf: &[u8]) -> usize;
		fn rust_writer_flush(writer: &mut RustWriter) -> bool;
	}

	unsafe extern "C++" {
		include!("cadrum/cpp/wrapper.h");

		// Opaque C++ types (accessed as cadrum::TopoDS_Shape etc. via using aliases)
		type TopoDS_Shape;
		type TopoDS_Face;
		type TopoDS_Edge;
		type TopExp_Explorer;
		type BooleanShape;

		// ==================== Shape I/O (streambuf callback) ====================

		// Plain STEP I/O — used only without `color` feature.
		// With color, STEP goes through XCAF (`read_step_color_stream` etc.).
		#[cfg(not(feature = "color"))]
		fn read_step_stream(reader: &mut RustReader) -> UniquePtr<TopoDS_Shape>;
		#[cfg(not(feature = "color"))]
		fn write_step_stream(shape: &TopoDS_Shape, writer: &mut RustWriter) -> bool;
		fn read_brep_bin_stream(reader: &mut RustReader) -> UniquePtr<TopoDS_Shape>;
		fn write_brep_bin_stream(shape: &TopoDS_Shape, writer: &mut RustWriter) -> bool;
		fn read_brep_text_stream(reader: &mut RustReader) -> UniquePtr<TopoDS_Shape>;
		fn write_brep_text_stream(shape: &TopoDS_Shape, writer: &mut RustWriter) -> bool;

		// ==================== Shape Constructors ====================

		fn make_half_space(ox: f64, oy: f64, oz: f64, nx: f64, ny: f64, nz: f64) -> UniquePtr<TopoDS_Shape>;

		fn make_box(x1: f64, y1: f64, z1: f64, x2: f64, y2: f64, z2: f64) -> UniquePtr<TopoDS_Shape>;

		fn make_cylinder(px: f64, py: f64, pz: f64, dx: f64, dy: f64, dz: f64, radius: f64, height: f64) -> UniquePtr<TopoDS_Shape>;

		fn make_sphere(cx: f64, cy: f64, cz: f64, radius: f64) -> UniquePtr<TopoDS_Shape>;

		fn make_cone(px: f64, py: f64, pz: f64, dx: f64, dy: f64, dz: f64, r1: f64, r2: f64, height: f64) -> UniquePtr<TopoDS_Shape>;

		fn make_torus(px: f64, py: f64, pz: f64, dx: f64, dy: f64, dz: f64, r1: f64, r2: f64) -> UniquePtr<TopoDS_Shape>;

		fn make_empty() -> UniquePtr<TopoDS_Shape>;

		fn deep_copy(shape: &TopoDS_Shape) -> UniquePtr<TopoDS_Shape>;
		fn shallow_copy(shape: &TopoDS_Shape) -> UniquePtr<TopoDS_Shape>;

		// ==================== Boolean Operations ====================

		// Unified boolean op. `op_kind`: 0 = fuse(union), 1 = cut(a − b), 2 = common(intersect).
		fn boolean_op(a: &TopoDS_Shape, b: &TopoDS_Shape, op_kind: u32) -> UniquePtr<BooleanShape>;

		fn boolean_shape_shape(r: &BooleanShape) -> UniquePtr<TopoDS_Shape>;
		fn boolean_shape_from_a(r: &BooleanShape) -> Vec<u64>;
		fn boolean_shape_from_b(r: &BooleanShape) -> Vec<u64>;

		// ==================== Colored STEP I/O (color feature only) ====================

		#[cfg(feature = "color")]
		type ColoredStepData;

		#[cfg(feature = "color")]
		fn read_step_color_stream(reader: &mut RustReader) -> UniquePtr<ColoredStepData>;
		#[cfg(feature = "color")]
		fn colored_step_shape(d: &ColoredStepData) -> UniquePtr<TopoDS_Shape>;
		#[cfg(feature = "color")]
		fn colored_step_ids(d: &ColoredStepData) -> Vec<u64>;
		#[cfg(feature = "color")]
		fn colored_step_colors_r(d: &ColoredStepData) -> Vec<f32>;
		#[cfg(feature = "color")]
		fn colored_step_colors_g(d: &ColoredStepData) -> Vec<f32>;
		#[cfg(feature = "color")]
		fn colored_step_colors_b(d: &ColoredStepData) -> Vec<f32>;

		#[cfg(feature = "color")]
		fn write_step_color_stream(shape: &TopoDS_Shape, ids: &[u64], cr: &[f32], cg: &[f32], cb: &[f32], writer: &mut RustWriter) -> bool;

		// ==================== Shape Methods ====================

		// Plain clean — used only without `color` feature.
		// With color, clean goes through `clean_shape_full` to remap face IDs.
		#[cfg(not(feature = "color"))]
		fn clean_shape(shape: &TopoDS_Shape) -> UniquePtr<TopoDS_Shape>;

		#[cfg(feature = "color")]
		type CleanShape;
		#[cfg(feature = "color")]
		fn clean_shape_full(shape: &TopoDS_Shape) -> UniquePtr<CleanShape>;
		#[cfg(feature = "color")]
		fn clean_shape_get(r: &CleanShape) -> UniquePtr<TopoDS_Shape>;
		#[cfg(feature = "color")]
		fn clean_shape_mapping(r: &CleanShape) -> Vec<u64>;

		fn translate_shape(shape: &TopoDS_Shape, tx: f64, ty: f64, tz: f64) -> UniquePtr<TopoDS_Shape>;

		fn rotate_shape(shape: &TopoDS_Shape, ox: f64, oy: f64, oz: f64, dx: f64, dy: f64, dz: f64, angle: f64) -> UniquePtr<TopoDS_Shape>;

		fn scale_shape(shape: &TopoDS_Shape, cx: f64, cy: f64, cz: f64, factor: f64) -> UniquePtr<TopoDS_Shape>;

		fn mirror_shape(shape: &TopoDS_Shape, ox: f64, oy: f64, oz: f64, nx: f64, ny: f64, nz: f64) -> UniquePtr<TopoDS_Shape>;

		fn shape_is_null(shape: &TopoDS_Shape) -> bool;
		fn shape_is_solid(shape: &TopoDS_Shape) -> bool;
		fn shape_shell_count(shape: &TopoDS_Shape) -> u32;
		fn shape_volume(shape: &TopoDS_Shape) -> f64;
		fn shape_contains_point(shape: &TopoDS_Shape, x: f64, y: f64, z: f64) -> bool;
		fn shape_bounding_box(shape: &TopoDS_Shape, xmin: &mut f64, ymin: &mut f64, zmin: &mut f64, xmax: &mut f64, ymax: &mut f64, zmax: &mut f64);

		// ==================== Compound Decompose/Compose ====================

		fn decompose_into_solids(shape: &TopoDS_Shape) -> UniquePtr<CxxVector<TopoDS_Shape>>;
		fn compound_add(compound: Pin<&mut TopoDS_Shape>, child: &TopoDS_Shape);

		// ==================== Meshing ====================

		fn mesh_shape(shape: &TopoDS_Shape, tolerance: f64) -> MeshData;

		// ==================== Explorer / Iterators ====================

		fn explore_faces(shape: &TopoDS_Shape) -> UniquePtr<TopExp_Explorer>;
		fn explore_edges(shape: &TopoDS_Shape) -> UniquePtr<TopExp_Explorer>;

		fn explorer_more(explorer: &TopExp_Explorer) -> bool;
		fn explorer_next(explorer: Pin<&mut TopExp_Explorer>);

		fn explorer_current_face(explorer: &TopExp_Explorer) -> UniquePtr<TopoDS_Face>;
		fn explorer_current_edge(explorer: &TopExp_Explorer) -> UniquePtr<TopoDS_Edge>;

		// ==================== Face Methods ====================

		fn face_tshape_id(face: &TopoDS_Face) -> u64;
		fn shape_tshape_id(shape: &TopoDS_Shape) -> u64;

		// ==================== Edge Methods ====================

		fn edge_approximation_segments(edge: &TopoDS_Edge, tolerance: f64) -> ApproxPoints;
		fn edge_approximation_segments_ex(edge: &TopoDS_Edge, angular: f64, chord: f64) -> ApproxPoints;

		fn make_helix_edge(ax: f64, ay: f64, az: f64, xrx: f64, xry: f64, xrz: f64, radius: f64, pitch: f64, height: f64) -> UniquePtr<TopoDS_Edge>;
		fn make_polygon_edges(coords: &[f64]) -> UniquePtr<CxxVector<TopoDS_Edge>>;
		fn make_circle_edge(ax: f64, ay: f64, az: f64, radius: f64) -> UniquePtr<TopoDS_Edge>;
		fn make_line_edge(ax: f64, ay: f64, az: f64, bx: f64, by: f64, bz: f64) -> UniquePtr<TopoDS_Edge>;
		fn make_arc_edge(sx: f64, sy: f64, sz: f64, mx: f64, my: f64, mz: f64, ex: f64, ey: f64, ez: f64) -> UniquePtr<TopoDS_Edge>;
		fn make_bspline_edge(coords: &[f64], end_kind: u32, sx: f64, sy: f64, sz: f64, ex: f64, ey: f64, ez: f64) -> UniquePtr<TopoDS_Edge>;

		fn edge_start_point(edge: &TopoDS_Edge, x: &mut f64, y: &mut f64, z: &mut f64);
		fn edge_start_tangent(edge: &TopoDS_Edge, x: &mut f64, y: &mut f64, z: &mut f64);
		fn edge_is_closed(edge: &TopoDS_Edge) -> bool;

		fn deep_copy_edge(edge: &TopoDS_Edge) -> UniquePtr<TopoDS_Edge>;

		fn translate_edge(edge: &TopoDS_Edge, tx: f64, ty: f64, tz: f64) -> UniquePtr<TopoDS_Edge>;
		fn rotate_edge(edge: &TopoDS_Edge, ox: f64, oy: f64, oz: f64, dx: f64, dy: f64, dz: f64, angle: f64) -> UniquePtr<TopoDS_Edge>;
		fn scale_edge(edge: &TopoDS_Edge, cx: f64, cy: f64, cz: f64, factor: f64) -> UniquePtr<TopoDS_Edge>;
		fn mirror_edge(edge: &TopoDS_Edge, ox: f64, oy: f64, oz: f64, nx: f64, ny: f64, nz: f64) -> UniquePtr<TopoDS_Edge>;

		fn make_extrude(profile_edges: &CxxVector<TopoDS_Edge>, dx: f64, dy: f64, dz: f64) -> UniquePtr<TopoDS_Shape>;
		fn make_pipe_shell(all_edges: &CxxVector<TopoDS_Edge>, spine_edges: &CxxVector<TopoDS_Edge>, orient: u32, ux: f64, uy: f64, uz: f64, aux_spine_edges: &CxxVector<TopoDS_Edge>) -> UniquePtr<TopoDS_Shape>;
		fn make_loft(all_edges: &CxxVector<TopoDS_Edge>) -> UniquePtr<TopoDS_Shape>;
		fn make_gordon(profile_edges: &CxxVector<TopoDS_Edge>, guide_edges: &CxxVector<TopoDS_Edge>) -> UniquePtr<TopoDS_Shape>;

		fn edge_vec_new() -> UniquePtr<CxxVector<TopoDS_Edge>>;
		fn edge_vec_push(v: Pin<&mut CxxVector<TopoDS_Edge>>, e: &TopoDS_Edge);
		fn edge_vec_push_null(v: Pin<&mut CxxVector<TopoDS_Edge>>);
	}
}

// Re-export all bridge items so other modules can use `ffi::TopoDS_Shape` etc.
pub use ffi_bridge::*;

// cxx opaque types default to `!Send + !Sync`. We mark them `Send` here so
// that `UniquePtr<TopoDS_Shape>` (and friends) become `Send`, which in turn
// makes our wrapper types (`Shape`, `Solid`, `Face`, `Edge`) auto-Send.
//
// Safety rationale:
//   - `UniquePtr` gives exclusive ownership — no aliasing is possible.
//   - These values are never shared across threads simultaneously; they are
//     only *moved* to another thread, which is what `Send` permits.
//   - `Sync` is intentionally NOT implemented: OCC's `Handle<Geom_XXX>`
//     reference counts are non-atomic, so concurrent `&T` access across
//     threads would be unsound.
unsafe impl Send for TopoDS_Shape {}
unsafe impl Send for TopoDS_Face {}
unsafe impl Send for TopoDS_Edge {}
unsafe impl Send for TopExp_Explorer {}
unsafe impl Send for BooleanShape {}
#[cfg(feature = "color")]
unsafe impl Send for CleanShape {}
#[cfg(feature = "color")]
unsafe impl Send for ColoredStepData {}

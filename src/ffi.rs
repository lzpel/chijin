use crate::stream::{rust_reader_read, rust_writer_flush, rust_writer_write};
use crate::stream::{RustReader, RustWriter};

#[cxx::bridge(namespace = "chijin")]
mod ffi_bridge {
	// Shared struct for mesh data returned from C++
	struct MeshData {
		vertices: Vec<f64>,         // flat xyz
		uvs: Vec<f64>,              // flat uv
		normals: Vec<f64>,          // flat xyz
		indices: Vec<u32>,
		face_tshape_ids: Vec<u64>,  // per-triangle TShape* address
		success: bool,
	}

	// Shared struct for approximation points
	struct ApproxPoints {
		coords: Vec<f64>, // flat xyz
		count: u32,
	}

	// Shared struct for HLR projected edges (SVG export)
	struct SvgEdgeData {
		visible_coords: Vec<f64>,  // flat x,y pairs
		visible_counts: Vec<u32>,  // point count per polyline
		hidden_coords: Vec<f64>,
		hidden_counts: Vec<u32>,
		min_x: f64,
		min_y: f64,
		max_x: f64,
		max_y: f64,
		success: bool,
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
		include!("chijin/cpp/wrapper.h");

		// Opaque C++ types (accessed as chijin::TopoDS_Shape etc. via using aliases)
		type TopoDS_Shape;
		type TopoDS_Face;
		type TopoDS_Edge;
		type TopExp_Explorer;
		type BooleanShape;

		// ==================== Shape I/O (streambuf callback) ====================

		fn read_step_stream(reader: &mut RustReader) -> UniquePtr<TopoDS_Shape>;
		fn write_step_stream(shape: &TopoDS_Shape, writer: &mut RustWriter) -> bool;
		fn read_brep_bin_stream(reader: &mut RustReader) -> UniquePtr<TopoDS_Shape>;
		fn write_brep_bin_stream(shape: &TopoDS_Shape, writer: &mut RustWriter) -> bool;
		fn read_brep_text_stream(reader: &mut RustReader) -> UniquePtr<TopoDS_Shape>;
		fn write_brep_text_stream(shape: &TopoDS_Shape, writer: &mut RustWriter) -> bool;

		// ==================== Shape Constructors ====================

		fn make_half_space(
			ox: f64,
			oy: f64,
			oz: f64,
			nx: f64,
			ny: f64,
			nz: f64,
		) -> UniquePtr<TopoDS_Shape>;

		fn make_box(
			x1: f64,
			y1: f64,
			z1: f64,
			x2: f64,
			y2: f64,
			z2: f64,
		) -> UniquePtr<TopoDS_Shape>;

		fn make_cylinder(
			px: f64,
			py: f64,
			pz: f64,
			dx: f64,
			dy: f64,
			dz: f64,
			radius: f64,
			height: f64,
		) -> UniquePtr<TopoDS_Shape>;

		fn make_empty() -> UniquePtr<TopoDS_Shape>;

		fn deep_copy(shape: &TopoDS_Shape) -> UniquePtr<TopoDS_Shape>;
		fn shallow_copy(shape: &TopoDS_Shape) -> UniquePtr<TopoDS_Shape>;

		// ==================== Boolean Operations ====================

		fn boolean_fuse(a: &TopoDS_Shape, b: &TopoDS_Shape) -> UniquePtr<BooleanShape>;
		fn boolean_cut(a: &TopoDS_Shape, b: &TopoDS_Shape) -> UniquePtr<BooleanShape>;
		fn boolean_common(a: &TopoDS_Shape, b: &TopoDS_Shape) -> UniquePtr<BooleanShape>;

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
		fn write_step_color_stream(
			shape: &TopoDS_Shape,
			ids: &[u64],
			cr: &[f32],
			cg: &[f32],
			cb: &[f32],
			writer: &mut RustWriter,
		) -> bool;

		// ==================== Shape Methods ====================

		fn clean_shape(shape: &TopoDS_Shape) -> UniquePtr<TopoDS_Shape>;

		#[cfg(feature = "color")]
		type CleanShape;
		#[cfg(feature = "color")]
		fn clean_shape_full(shape: &TopoDS_Shape) -> UniquePtr<CleanShape>;
		#[cfg(feature = "color")]
		fn clean_shape_get(r: &CleanShape) -> UniquePtr<TopoDS_Shape>;
		#[cfg(feature = "color")]
		fn clean_shape_mapping(r: &CleanShape) -> Vec<u64>;

		fn translate_shape(
			shape: &TopoDS_Shape,
			tx: f64,
			ty: f64,
			tz: f64,
		) -> UniquePtr<TopoDS_Shape>;

		fn rotate_shape(
			shape: &TopoDS_Shape,
			ox: f64, oy: f64, oz: f64,
			dx: f64, dy: f64, dz: f64,
			angle: f64,
		) -> UniquePtr<TopoDS_Shape>;

		fn scale_shape(
			shape: &TopoDS_Shape,
			cx: f64, cy: f64, cz: f64,
			factor: f64,
		) -> UniquePtr<TopoDS_Shape>;

		fn shape_is_null(shape: &TopoDS_Shape) -> bool;
		fn shape_is_solid(shape: &TopoDS_Shape) -> bool;
		fn shape_shell_count(shape: &TopoDS_Shape) -> u32;
		fn shape_volume(shape: &TopoDS_Shape) -> f64;
		fn shape_contains_point(shape: &TopoDS_Shape, x: f64, y: f64, z: f64) -> bool;

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
		fn face_from_polygon(coords: &[f64]) -> UniquePtr<TopoDS_Face>;
		fn face_center_of_mass(face: &TopoDS_Face, cx: &mut f64, cy: &mut f64, cz: &mut f64);
		fn face_normal_at_center(face: &TopoDS_Face, nx: &mut f64, ny: &mut f64, nz: &mut f64);
		fn face_extrude(face: &TopoDS_Face, dx: f64, dy: f64, dz: f64) -> UniquePtr<TopoDS_Shape>;
		fn face_revolve(
			face: &TopoDS_Face,
			ox: f64, oy: f64, oz: f64,
			dx: f64, dy: f64, dz: f64,
			angle: f64,
		) -> UniquePtr<TopoDS_Shape>;
		fn face_helix(
			face: &TopoDS_Face,
			ox: f64, oy: f64, oz: f64,
			dx: f64, dy: f64, dz: f64,
			pitch: f64, turns: f64,
			align_to_spine: bool,
		) -> UniquePtr<TopoDS_Shape>;

		// ==================== Edge Methods ====================

		fn edge_approximation_segments(edge: &TopoDS_Edge, tolerance: f64) -> ApproxPoints;
		fn edge_approximation_segments_ex(
			edge: &TopoDS_Edge,
			angular: f64,
			chord: f64,
		) -> ApproxPoints;

		// ==================== SVG / HLR Projection ====================

		fn project_shape_hlr(
			shape: &TopoDS_Shape,
			dx: f64, dy: f64, dz: f64,
			tolerance: f64,
		) -> SvgEdgeData;
	}
}

// Re-export all bridge items so other modules can use `crate::ffi::TopoDS_Shape` etc.
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

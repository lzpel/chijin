#[cxx::bridge(namespace = "chijin")]
pub mod ffi {
	// Shared struct for mesh data returned from C++
	struct MeshData {
		vertices: Vec<f64>, // flat xyz
		uvs: Vec<f64>,      // flat uv
		normals: Vec<f64>,  // flat xyz
		indices: Vec<u32>,
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
		include!("chijin/cpp/wrapper.h");

		// Opaque C++ types
		type TopoDS_Shape;
		type TopoDS_Face;
		type TopoDS_Edge;
		type TopExp_Explorer;

		// ==================== Shape I/O (streambuf callback) ====================

		fn read_step_stream(reader: &mut RustReader) -> UniquePtr<TopoDS_Shape>;
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

		// ==================== Boolean Operations ====================

		fn boolean_fuse(a: &TopoDS_Shape, b: &TopoDS_Shape) -> UniquePtr<TopoDS_Shape>;
		fn boolean_cut(a: &TopoDS_Shape, b: &TopoDS_Shape) -> UniquePtr<TopoDS_Shape>;
		fn boolean_common(a: &TopoDS_Shape, b: &TopoDS_Shape) -> UniquePtr<TopoDS_Shape>;

		// ==================== Shape Methods ====================

		fn clean_shape(shape: &TopoDS_Shape) -> UniquePtr<TopoDS_Shape>;

		fn translate_shape(
			shape: &TopoDS_Shape,
			tx: f64,
			ty: f64,
			tz: f64,
		) -> UniquePtr<TopoDS_Shape>;

		fn shape_is_null(shape: &TopoDS_Shape) -> bool;

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

		fn face_center_of_mass(face: &TopoDS_Face, cx: &mut f64, cy: &mut f64, cz: &mut f64);
		fn face_normal_at_center(face: &TopoDS_Face, nx: &mut f64, ny: &mut f64, nz: &mut f64);
		fn face_extrude(face: &TopoDS_Face, dx: f64, dy: f64, dz: f64) -> UniquePtr<TopoDS_Shape>;
		fn face_to_shape(face: &TopoDS_Face) -> UniquePtr<TopoDS_Shape>;

		// ==================== Edge Methods ====================

		fn edge_approximation_segments(edge: &TopoDS_Edge, tolerance: f64) -> ApproxPoints;
	}
}

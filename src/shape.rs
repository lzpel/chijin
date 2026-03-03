use crate::error::Error;
use crate::ffi;
use crate::iterators::{EdgeIterator, FaceIterator};
use crate::mesh::Mesh;
use crate::stream::{RustReader, RustWriter};
use glam::{DVec2, DVec3};
use std::io::{Read, Write};

/// Result of a boolean operation.
///
/// `new_faces` is a compound of the faces generated at the tool boundary:
/// - For [`intersect`](Shape::intersect) and [`subtract`](Shape::subtract):
///   the cross-section faces at the cut plane.
/// - For [`union`](Shape::union): an empty compound (no new cut faces are generated).
///
/// Both fields are `pub` for direct access. Use [`From<BooleanShape> for Shape`]
/// (`.into()`) when only the shape is needed.
pub struct BooleanShape {
	pub shape: Shape,
	pub new_faces: Shape,
}

impl From<BooleanShape> for Shape {
	fn from(r: BooleanShape) -> Shape {
		r.shape
	}
}

/// A topological shape wrapping `TopoDS_Shape`.
///
/// This is the central type in Chijin. Shapes can represent solids, compounds,
/// faces, edges, or any other topology supported by OpenCASCADE.
///
/// When the `color` feature is enabled, shapes additionally carry a per-face
/// color map that is automatically relayed through boolean operations,
/// `clean()`, and `translated()`.
pub struct Shape {
	pub(crate) inner: cxx::UniquePtr<ffi::TopoDS_Shape>,
	#[cfg(feature = "color")]
	pub(crate) colors: cxx::UniquePtr<crate::color_ffi::ColorMap>,
}

// `Shape` is `Send` because `UniquePtr<TopoDS_Shape>` is `Send`
// (see ffi.rs — `unsafe impl Send for TopoDS_Shape`).
// `Sync` is intentionally NOT implemented: OCC Handle<> ref-counts are
// non-atomic, making concurrent `&Shape` access from multiple threads unsound.

/// Helper macro to construct a Shape with or without the color field.
#[cfg(feature = "color")]
macro_rules! shape_new {
	($inner:expr) => {
		Shape {
			inner: $inner,
			colors: crate::color_ffi::colormap_new(),
		}
	};
	($inner:expr, $colors:expr) => {
		Shape {
			inner: $inner,
			colors: $colors,
		}
	};
}

#[cfg(not(feature = "color"))]
macro_rules! shape_new {
	($inner:expr) => {
		Shape { inner: $inner }
	};
	($inner:expr, $_colors:expr) => {
		Shape { inner: $inner }
	};
}

// ==================== Constructors ====================

impl Shape {
	/// Read a shape from a STEP format stream.
	///
	/// Accepts any `impl Read` (file, network stream, `&[u8]`, etc.).
	/// Data is streamed chunk-by-chunk via a C++ `std::streambuf` bridge —
	/// the entire content is never buffered in memory.
	///
	/// # Bug 2 fix
	/// The `STEPControl_Reader` is leaked in the C++ layer to prevent
	/// `STATUS_ACCESS_VIOLATION` on process exit.
	///
	/// # Errors
	/// Returns [`Error::StepReadFailed`] if the data cannot be parsed.
	pub fn read_step(reader: &mut impl Read) -> Result<Shape, Error> {
		let mut rust_reader = RustReader::from_ref(reader);
		let inner = ffi::read_step_stream(&mut rust_reader);
		if inner.is_null() {
			return Err(Error::StepReadFailed);
		}
		Ok(shape_new!(inner))
	}

	/// Read a shape from a BRep binary format stream.
	///
	/// # Errors
	/// Returns [`Error::BrepReadFailed`] if the data cannot be parsed.
	pub fn read_brep_bin(reader: &mut impl Read) -> Result<Shape, Error> {
		let mut rust_reader = RustReader::from_ref(reader);
		let inner = ffi::read_brep_bin_stream(&mut rust_reader);
		if inner.is_null() {
			return Err(Error::BrepReadFailed);
		}
		Ok(shape_new!(inner))
	}

	/// Write this shape in STEP format to a stream.
	///
	/// # Errors
	/// Returns [`Error::StepWriteFailed`] if writing fails.
	pub fn write_step(&self, writer: &mut impl Write) -> Result<(), Error> {
		let mut rust_writer = RustWriter::from_ref(writer);
		if ffi::write_step_stream(&self.inner, &mut rust_writer) {
			Ok(())
		} else {
			Err(Error::StepWriteFailed)
		}
	}

	/// Write this shape in BRep binary format to a stream.
	///
	/// # Errors
	/// Returns [`Error::BrepWriteFailed`] if writing fails.
	pub fn write_brep_bin(&self, writer: &mut impl Write) -> Result<(), Error> {
		let mut rust_writer = RustWriter::from_ref(writer);
		if ffi::write_brep_bin_stream(&self.inner, &mut rust_writer) {
			Ok(())
		} else {
			Err(Error::BrepWriteFailed)
		}
	}

	/// Read a shape from a BRep text format stream.
	///
	/// # Errors
	/// Returns [`Error::BrepReadFailed`] if the data cannot be parsed.
	pub fn read_brep_text(reader: &mut impl Read) -> Result<Shape, Error> {
		let mut rust_reader = RustReader::from_ref(reader);
		let inner = ffi::read_brep_text_stream(&mut rust_reader);
		if inner.is_null() {
			return Err(Error::BrepReadFailed);
		}
		Ok(shape_new!(inner))
	}

	/// Write this shape in BRep text format to a stream.
	///
	/// # Errors
	/// Returns [`Error::BrepWriteFailed`] if writing fails.
	pub fn write_brep_text(&self, writer: &mut impl Write) -> Result<(), Error> {
		let mut rust_writer = RustWriter::from_ref(writer);
		if ffi::write_brep_text_stream(&self.inner, &mut rust_writer) {
			Ok(())
		} else {
			Err(Error::BrepWriteFailed)
		}
	}

	/// Create a half-space solid.
	///
	/// The solid fills the half-space on the side **where the normal points**.
	/// When used with `shape.intersect(&half_space)`, the portion on the
	/// `plane_normal` side is retained.
	///
	/// The reference point is placed opposite to the normal direction,
	/// so the solid represents the space in the normal's direction.
	pub fn half_space(plane_origin: DVec3, plane_normal: DVec3) -> Shape {
		let inner = ffi::make_half_space(
			plane_origin.x,
			plane_origin.y,
			plane_origin.z,
			plane_normal.x,
			plane_normal.y,
			plane_normal.z,
		);
		shape_new!(inner)
	}

	/// Create a box from two opposite corner points.
	///
	/// The corners are normalized internally (min/max), so the order
	/// of the points does not matter.
	pub fn box_from_corners(corner_1: DVec3, corner_2: DVec3) -> Shape {
		let inner = ffi::make_box(
			corner_1.x, corner_1.y, corner_1.z, corner_2.x, corner_2.y, corner_2.z,
		);
		shape_new!(inner)
	}

	/// Create a cylinder.
	///
	/// - `p`: center of the base circle
	/// - `r`: radius
	/// - `dir`: axis direction
	/// - `h`: height along the axis
	pub fn cylinder(p: DVec3, r: f64, dir: DVec3, h: f64) -> Shape {
		let inner = ffi::make_cylinder(p.x, p.y, p.z, dir.x, dir.y, dir.z, r, h);
		shape_new!(inner)
	}

	/// Create an empty compound shape.
	///
	/// Uses `TopoDS_Compound` + `BRep_Builder::MakeCompound` instead of
	/// a null shape, because null shapes cause boolean operations to fail.
	pub fn empty() -> Shape {
		let inner = ffi::make_empty();
		shape_new!(inner)
	}

	/// Create an independent deep copy of this shape.
	///
	/// Uses `BRepBuilderAPI_Copy` to create a complete copy that shares
	/// no internal `Handle<Geom_XXX>` references with the original.
	pub fn deep_copy(&self) -> Shape {
		let inner = ffi::deep_copy(&self.inner);
		#[cfg(feature = "color")]
		{
			let colors =
				crate::color_ffi::remap_colors_after_copy(&self.inner, &inner, &self.colors);
			return shape_new!(inner, colors);
		}
		#[cfg(not(feature = "color"))]
		shape_new!(inner)
	}
}

// ==================== Boolean Operations ====================

impl Shape {
	/// Boolean union (fuse) with another shape.
	///
	/// Returns a [`BooleanShape`] whose `new_faces` is an empty compound
	/// (union has no tool boundary that generates new faces).
	///
	/// # Bug 1 fix
	/// The result is automatically deep-copied in the C++ layer via
	/// `BRepBuilderAPI_Copy` to prevent `STATUS_HEAP_CORRUPTION`
	/// when shapes are dropped in any order.
	pub fn union(&self, other: &Shape) -> Result<BooleanShape, Error> {
		#[cfg(feature = "color")]
		{
			let r = crate::color_ffi::boolean_fuse_colored(
				&self.inner,
				&self.colors,
				&other.inner,
				&other.colors,
			);
			if r.is_null() {
				return Err(Error::BooleanOperationFailed);
			}
			return Ok(BooleanShape {
				shape: shape_new!(
					crate::color_ffi::colored_result_shape(&r),
					crate::color_ffi::colored_result_shape_colors(&r)
				),
				new_faces: shape_new!(
					crate::color_ffi::colored_result_new_faces(&r),
					crate::color_ffi::colored_result_new_faces_colors(&r)
				),
			});
		}
		#[cfg(not(feature = "color"))]
		{
			let r = ffi::boolean_fuse(&self.inner, &other.inner);
			if r.is_null() {
				return Err(Error::BooleanOperationFailed);
			}
			Ok(BooleanShape {
				shape: shape_new!(ffi::boolean_shape_shape(&r)),
				new_faces: shape_new!(ffi::boolean_shape_new_faces(&r)),
			})
		}
	}

	/// Boolean subtraction (cut) with another shape.
	///
	/// `new_faces` contains the cross-section faces generated at the tool boundary.
	///
	/// See [`union`](Self::union) for details on automatic deep-copy.
	pub fn subtract(&self, other: &Shape) -> Result<BooleanShape, Error> {
		#[cfg(feature = "color")]
		{
			let r = crate::color_ffi::boolean_cut_colored(
				&self.inner,
				&self.colors,
				&other.inner,
				&other.colors,
			);
			if r.is_null() {
				return Err(Error::BooleanOperationFailed);
			}
			return Ok(BooleanShape {
				shape: shape_new!(
					crate::color_ffi::colored_result_shape(&r),
					crate::color_ffi::colored_result_shape_colors(&r)
				),
				new_faces: shape_new!(
					crate::color_ffi::colored_result_new_faces(&r),
					crate::color_ffi::colored_result_new_faces_colors(&r)
				),
			});
		}
		#[cfg(not(feature = "color"))]
		{
			let r = ffi::boolean_cut(&self.inner, &other.inner);
			if r.is_null() {
				return Err(Error::BooleanOperationFailed);
			}
			Ok(BooleanShape {
				shape: shape_new!(ffi::boolean_shape_shape(&r)),
				new_faces: shape_new!(ffi::boolean_shape_new_faces(&r)),
			})
		}
	}

	/// Boolean intersection (common) with another shape.
	///
	/// `new_faces` contains the cross-section faces generated at the tool boundary.
	/// This is the primary source of cut faces used by the stretch algorithm.
	///
	/// See [`union`](Self::union) for details on automatic deep-copy.
	pub fn intersect(&self, other: &Shape) -> Result<BooleanShape, Error> {
		#[cfg(feature = "color")]
		{
			let r = crate::color_ffi::boolean_common_colored(
				&self.inner,
				&self.colors,
				&other.inner,
				&other.colors,
			);
			if r.is_null() {
				return Err(Error::BooleanOperationFailed);
			}
			return Ok(BooleanShape {
				shape: shape_new!(
					crate::color_ffi::colored_result_shape(&r),
					crate::color_ffi::colored_result_shape_colors(&r)
				),
				new_faces: shape_new!(
					crate::color_ffi::colored_result_new_faces(&r),
					crate::color_ffi::colored_result_new_faces_colors(&r)
				),
			});
		}
		#[cfg(not(feature = "color"))]
		{
			let r = ffi::boolean_common(&self.inner, &other.inner);
			if r.is_null() {
				return Err(Error::BooleanOperationFailed);
			}
			Ok(BooleanShape {
				shape: shape_new!(ffi::boolean_shape_shape(&r)),
				new_faces: shape_new!(ffi::boolean_shape_new_faces(&r)),
			})
		}
	}
}

// ==================== Shape Methods ====================

impl Shape {
	/// Clean the shape by unifying same-domain faces, edges, and vertices.
	///
	/// Uses `ShapeUpgrade_UnifySameDomain` to remove redundant topology
	/// created by boolean operations.
	pub fn clean(&self) -> Result<Shape, Error> {
		#[cfg(feature = "color")]
		{
			let mut out_colors = crate::color_ffi::colormap_new();
			let inner = crate::color_ffi::clean_shape_colored(
				&self.inner,
				&self.colors,
				out_colors.pin_mut(),
			);
			if inner.is_null() {
				return Err(Error::CleanFailed);
			}
			return Ok(shape_new!(inner, out_colors));
		}
		#[cfg(not(feature = "color"))]
		{
			let inner = ffi::clean_shape(&self.inner);
			if inner.is_null() {
				return Err(Error::CleanFailed);
			}
			Ok(shape_new!(inner))
		}
	}

	/// Create a new shape translated by the given vector.
	///
	/// # Bug 5 fix
	/// Uses `BRepBuilderAPI_Transform` which properly propagates the
	/// transformation to all sub-shapes, including those in compounds
	/// created by boolean operations.
	pub fn translated(&self, translation: DVec3) -> Shape {
		let inner = ffi::translate_shape(&self.inner, translation.x, translation.y, translation.z);
		#[cfg(feature = "color")]
		{
			// BRepBuilderAPI_Transform creates new TShape pointers,
			// so remap colors by face enumeration order.
			let colors =
				crate::color_ffi::remap_colors_after_copy(&self.inner, &inner, &self.colors);
			return shape_new!(inner, colors);
		}
		#[cfg(not(feature = "color"))]
		shape_new!(inner)
	}

	/// Set a global translation on this shape (in-place mutation).
	///
	/// **Warning**: With `propagate=false`, this only updates the root shape's
	/// `TopLoc_Location` and does **not** affect sub-shapes in compounds.
	/// Prefer [`translated`](Self::translated) for compound shapes.
	pub fn set_global_translation(&mut self, translation: DVec3) {
		// Replace self with a translated copy for correctness
		let translated =
			ffi::translate_shape(&self.inner, translation.x, translation.y, translation.z);
		#[cfg(feature = "color")]
		{
			self.colors =
				crate::color_ffi::remap_colors_after_copy(&self.inner, &translated, &self.colors);
		}
		self.inner = translated;
	}

	/// Mesh this shape with the given linear deflection tolerance.
	///
	/// # Bug 3 fix
	/// The normals array now has exactly the same length as the vertices
	/// array (previous binding had an off-by-one error).
	///
	/// # Errors
	/// Returns [`Error::TriangulationFailed`] if meshing fails.
	pub fn mesh_with_tolerance(&self, tol: f64) -> Result<Mesh, Error> {
		let data = ffi::mesh_shape(&self.inner, tol);
		if !data.success {
			return Err(Error::TriangulationFailed);
		}

		let vertex_count = data.vertices.len() / 3;

		let vertices: Vec<DVec3> = (0..vertex_count)
			.map(|i| {
				DVec3::new(
					data.vertices[i * 3],
					data.vertices[i * 3 + 1],
					data.vertices[i * 3 + 2],
				)
			})
			.collect();

		let uvs: Vec<DVec2> = (0..vertex_count)
			.map(|i| DVec2::new(data.uvs[i * 2], data.uvs[i * 2 + 1]))
			.collect();

		let normals: Vec<DVec3> = (0..vertex_count)
			.map(|i| {
				DVec3::new(
					data.normals[i * 3],
					data.normals[i * 3 + 1],
					data.normals[i * 3 + 2],
				)
			})
			.collect();

		let indices: Vec<usize> = data.indices.iter().map(|&i| i as usize).collect();

		Ok(Mesh {
			vertices,
			uvs,
			normals,
			indices,
		})
	}

	/// Iterate over all faces in this shape.
	pub fn faces(&self) -> FaceIterator {
		let explorer = ffi::explore_faces(&self.inner);
		FaceIterator::new(explorer)
	}

	/// Iterate over all edges in this shape.
	pub fn edges(&self) -> EdgeIterator {
		let explorer = ffi::explore_edges(&self.inner);
		EdgeIterator::new(explorer)
	}

	/// Check if this shape is null.
	pub fn is_null(&self) -> bool {
		ffi::shape_is_null(&self.inner)
	}

	/// Count the number of shells in this shape.
	///
	/// Uses `TopExp_Explorer` with `TopAbs_SHELL`, which recursively
	/// traverses the entire shape tree. Returns 1 for a single solid,
	/// and N for a compound of N solids.
	pub fn shell_count(&self) -> u32 {
		ffi::shape_shell_count(&self.inner)
	}
}

// ==================== Color API (feature = "color") ====================

#[cfg(feature = "color")]
impl Shape {
	/// Set the color of a specific face.
	///
	/// The face must belong to this shape (obtained via [`faces()`](Self::faces)).
	/// Uses `IsSame()` internally for face lookup.
	pub fn set_face_color(&mut self, face: &crate::Face, rgb: [u8; 3]) {
		crate::color_ffi::colormap_set(self.colors.pin_mut(), &face.inner, rgb[0], rgb[1], rgb[2]);
	}

	/// Set the same color for all faces in this shape.
	pub fn set_all_faces_color(&mut self, rgb: [u8; 3]) {
		for face in self.faces() {
			crate::color_ffi::colormap_set(
				self.colors.pin_mut(),
				&face.inner,
				rgb[0],
				rgb[1],
				rgb[2],
			);
		}
	}

	/// Get the color of a specific face, if set.
	///
	/// Returns `None` if no color has been assigned to this face.
	pub fn face_color(&self, face: &crate::Face) -> Option<[u8; 3]> {
		let mut r = 0u8;
		let mut g = 0u8;
		let mut b = 0u8;
		if crate::color_ffi::colormap_get(&self.colors, &face.inner, &mut r, &mut g, &mut b) {
			Some([r, g, b])
		} else {
			None
		}
	}

	/// Get the number of face-color entries in the color map.
	pub fn color_count(&self) -> i32 {
		crate::color_ffi::colormap_size(&self.colors)
	}

	// ==================== XDE STEP colored I/O ====================

	/// Read a STEP stream with XDE color support.
	///
	/// Uses `STEPCAFControl_Reader` to parse the file and extract per-face
	/// colors stored as `STYLED_ITEM` entries. The returned `Shape` carries
	/// those colors in its color map.
	///
	/// Falls back cleanly when the file has no color annotations — the
	/// returned `Shape` will have an empty color map.
	///
	/// # Errors
	/// Returns [`Error::StepReadFailed`] if the data cannot be parsed.
	pub fn read_step_colored(reader: &mut impl Read) -> Result<Shape, Error> {
		// Buffer the entire stream: STEPCAFControl_Reader needs seekable data.
		let mut data = Vec::new();
		reader.read_to_end(&mut data).map_err(|_| Error::StepReadFailed)?;
		let mut out_colors = crate::color_ffi::colormap_new();
		let inner = crate::color_ffi::read_step_colored_from_slice(
			data.as_slice(),
			out_colors.pin_mut(),
		);
		if inner.is_null() {
			return Err(Error::StepReadFailed);
		}
		Ok(shape_new!(inner, out_colors))
	}

	/// Write this shape to a STEP stream with face colors via XDE.
	///
	/// Uses `STEPCAFControl_Writer` to create a STEP file that stores each
	/// face color as a `STYLED_ITEM` referencing `SURFACE_SIDE_STYLE`. The
	/// resulting file can be read back with [`read_step_colored`](Self::read_step_colored)
	/// or viewed in any XDE-capable CAD tool (FreeCAD, CATIA, etc.).
	///
	/// # Errors
	/// Returns [`Error::StepWriteFailed`] if writing fails.
	pub fn write_step_colored(&self, writer: &mut impl Write) -> Result<(), Error> {
		let bytes = crate::color_ffi::write_step_colored_to_vec(&self.inner, &self.colors);
		if bytes.is_empty() {
			return Err(Error::StepWriteFailed);
		}
		writer.write_all(&bytes).map_err(|_| Error::StepWriteFailed)
	}
}

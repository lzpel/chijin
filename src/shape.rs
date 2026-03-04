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
pub struct Shape {
	pub(crate) inner: cxx::UniquePtr<ffi::TopoDS_Shape>,
}

// `Shape` is `Send` because `UniquePtr<TopoDS_Shape>` is `Send`
// (see ffi.rs — `unsafe impl Send for TopoDS_Shape`).
// `Sync` is intentionally NOT implemented: OCC Handle<> ref-counts are
// non-atomic, making concurrent `&Shape` access from multiple threads unsound.

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
		Ok(Shape { inner })
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
		Ok(Shape { inner })
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
		Ok(Shape { inner })
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
		Shape { inner }
	}

	/// Create a box from two opposite corner points.
	///
	/// The corners are normalized internally (min/max), so the order
	/// of the points does not matter.
	pub fn box_from_corners(corner_1: DVec3, corner_2: DVec3) -> Shape {
		let inner = ffi::make_box(
			corner_1.x, corner_1.y, corner_1.z, corner_2.x, corner_2.y, corner_2.z,
		);
		Shape { inner }
	}

	/// Create a cylinder.
	///
	/// - `p`: center of the base circle
	/// - `r`: radius
	/// - `dir`: axis direction
	/// - `h`: height along the axis
	pub fn cylinder(p: DVec3, r: f64, dir: DVec3, h: f64) -> Shape {
		let inner = ffi::make_cylinder(p.x, p.y, p.z, dir.x, dir.y, dir.z, r, h);
		Shape { inner }
	}

	/// Create an empty compound shape.
	///
	/// Uses `TopoDS_Compound` + `BRep_Builder::MakeCompound` instead of
	/// a null shape, because null shapes cause boolean operations to fail.
	pub fn empty() -> Shape {
		let inner = ffi::make_empty();
		Shape { inner }
	}

	/// Create an independent deep copy of this shape.
	///
	/// Uses `BRepBuilderAPI_Copy` to create a complete copy that shares
	/// no internal `Handle<Geom_XXX>` references with the original.
	pub fn deep_copy(&self) -> Shape {
		let inner = ffi::deep_copy(&self.inner);
		Shape { inner }
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
		let r = ffi::boolean_fuse(&self.inner, &other.inner);
		if r.is_null() {
			return Err(Error::BooleanOperationFailed);
		}
		Ok(BooleanShape {
			shape: Shape { inner: ffi::boolean_shape_shape(&r) },
			new_faces: Shape { inner: ffi::boolean_shape_new_faces(&r) },
		})
	}

	/// Boolean subtraction (cut) with another shape.
	///
	/// `new_faces` contains the cross-section faces generated at the tool boundary.
	///
	/// See [`union`](Self::union) for details on automatic deep-copy.
	pub fn subtract(&self, other: &Shape) -> Result<BooleanShape, Error> {
		let r = ffi::boolean_cut(&self.inner, &other.inner);
		if r.is_null() {
			return Err(Error::BooleanOperationFailed);
		}
		Ok(BooleanShape {
			shape: Shape { inner: ffi::boolean_shape_shape(&r) },
			new_faces: Shape { inner: ffi::boolean_shape_new_faces(&r) },
		})
	}

	/// Boolean intersection (common) with another shape.
	///
	/// `new_faces` contains the cross-section faces generated at the tool boundary.
	/// This is the primary source of cut faces used by the stretch algorithm.
	///
	/// See [`union`](Self::union) for details on automatic deep-copy.
	pub fn intersect(&self, other: &Shape) -> Result<BooleanShape, Error> {
		let r = ffi::boolean_common(&self.inner, &other.inner);
		if r.is_null() {
			return Err(Error::BooleanOperationFailed);
		}
		Ok(BooleanShape {
			shape: Shape { inner: ffi::boolean_shape_shape(&r) },
			new_faces: Shape { inner: ffi::boolean_shape_new_faces(&r) },
		})
	}
}

// ==================== Shape Methods ====================

impl Shape {
	/// Clean the shape by unifying same-domain faces, edges, and vertices.
	///
	/// Uses `ShapeUpgrade_UnifySameDomain` to remove redundant topology
	/// created by boolean operations.
	pub fn clean(&self) -> Result<Shape, Error> {
		let inner = ffi::clean_shape(&self.inner);
		if inner.is_null() {
			return Err(Error::CleanFailed);
		}
		Ok(Shape { inner })
	}

	/// Create a new shape translated by the given vector.
	///
	/// # Bug 5 fix
	/// Uses `BRepBuilderAPI_Transform` which properly propagates the
	/// transformation to all sub-shapes, including those in compounds
	/// created by boolean operations.
	pub fn translated(&self, translation: DVec3) -> Shape {
		let inner = ffi::translate_shape(&self.inner, translation.x, translation.y, translation.z);
		Shape { inner }
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

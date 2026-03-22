use crate::error::Error;
use crate::ffi;
use crate::iterators::{EdgeIterator, FaceIterator};
use crate::mesh::Mesh;
use crate::stream::{RustReader, RustWriter};
use glam::{DVec2, DVec3};
use std::io::{Read, Write};

// ==================== Color types ====================

/// Identifier for a `TopoDS_TShape` object (pointer address).
///
/// Used as the key in `Shape::colormap` and in [`BooleanShape::new_face_ids`].
/// Valid as long as the owning `Shape` is alive.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TShapeId(pub u64);

/// RGB color with components in `0.0..=1.0`.
#[cfg(feature = "color")]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rgb {
	pub r: f32,
	pub g: f32,
	pub b: f32,
}

// ==================== BooleanShape ====================

/// Result of a boolean operation.
///
/// Use [`is_tool_face`](BooleanShape::is_tool_face) /
/// [`is_shape_face`](BooleanShape::is_shape_face) to classify faces of `shape`
/// by which operand they originated from.
///
/// Use [`From<BooleanShape> for Shape`] (`.into()`) when only the shape is needed.
pub struct BooleanShape {
	pub shape: Shape,
	from_a: Vec<u64>,
	from_b: Vec<u64>,
}

impl BooleanShape {
	/// Returns `true` if `face` originated from the `other` (tool) operand.
	///
	/// For `subtract` and `intersect` these are the cross-section / interface faces.
	///
	/// Implemented as a linear scan over `from_b`. post_ids are TShape* of the
	/// copied result, which never overlap with src_ids (original input pointers),
	/// so a flat `.contains()` on the interleaved `[post_id, src_id, ...]` array
	/// is correct.
	pub fn is_tool_face(&self, face: &crate::face::Face) -> bool {
		self.from_b.contains(&face.tshape_id().0)
	}

	/// Returns `true` if `face` originated from `self` (the base shape operand).
	pub fn is_shape_face(&self, face: &crate::face::Face) -> bool {
		self.from_a.contains(&face.tshape_id().0)
	}
}

impl From<BooleanShape> for Shape {
	fn from(r: BooleanShape) -> Shape {
		r.shape
	}
}

// ==================== Shape ====================

/// A topological shape wrapping `TopoDS_Shape`.
///
/// This is the central type in Chijin. Shapes can represent solids, compounds,
/// faces, edges, or any other topology supported by OpenCASCADE.
pub struct Shape {
	pub(crate) inner: cxx::UniquePtr<ffi::TopoDS_Shape>,
	/// Face-level color map. Key is [`TShapeId`] (the `TopoDS_TShape*` address).
	/// Only available when compiled with `--features color`.
	#[cfg(feature = "color")]
	pub colormap: std::collections::HashMap<TShapeId, Rgb>,
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
		Ok(Shape {
			inner,
			#[cfg(feature = "color")]
			colormap: std::collections::HashMap::new(),
		})
	}

	/// Read a STEP file and populate `colormap` with face colors found in the file.
	///
	/// Uses `STEPCAFControl_Reader` (XDE) which reads `STYLED_ITEM` / `COLOUR_RGB`
	/// records.  Faces without a color entry are simply absent from `colormap`.
	///
	/// # Errors
	/// Returns [`Error::StepReadFailed`] if the data cannot be parsed.
	#[cfg(feature = "color")]
	pub fn read_step_with_colors(reader: &mut impl Read) -> Result<Shape, Error> {
		let mut rust_reader = RustReader::from_ref(reader);
		let d = ffi::read_step_color_stream(&mut rust_reader);
		if d.is_null() {
			return Err(Error::StepReadFailed);
		}
		let inner = ffi::colored_step_shape(&d);
		if inner.is_null() {
			return Err(Error::StepReadFailed);
		}
		let ids = ffi::colored_step_ids(&d);
		let r = ffi::colored_step_colors_r(&d);
		let g = ffi::colored_step_colors_g(&d);
		let b = ffi::colored_step_colors_b(&d);
		let mut colormap = std::collections::HashMap::new();
		for i in 0..ids.len() {
			colormap.insert(TShapeId(ids[i]), Rgb { r: r[i], g: g[i], b: b[i] });
		}
		Ok(Shape { inner, colormap })
	}

	/// Write this shape in STEP format, embedding face colors from `colormap`.
	///
	/// Uses `STEPCAFControl_Writer` (XDE) to emit `STYLED_ITEM` / `COLOUR_RGB`
	/// records for every face present in `colormap`.
	///
	/// # Errors
	/// Returns [`Error::StepWriteFailed`] if writing fails.
	#[cfg(feature = "color")]
	pub fn write_step_with_colors(&self, writer: &mut impl Write) -> Result<(), Error> {
		let ids: Vec<u64> = self.colormap.keys().map(|k| k.0).collect();
		let r: Vec<f32> = ids.iter().map(|&id| self.colormap[&TShapeId(id)].r).collect();
		let g: Vec<f32> = ids.iter().map(|&id| self.colormap[&TShapeId(id)].g).collect();
		let b: Vec<f32> = ids.iter().map(|&id| self.colormap[&TShapeId(id)].b).collect();
		let mut rust_writer = RustWriter::from_ref(writer);
		if ffi::write_step_color_stream(&self.inner, &ids, &r, &g, &b, &mut rust_writer) {
			Ok(())
		} else {
			Err(Error::StepWriteFailed)
		}
	}

	/// Read a shape (with colors) from the CHJC binary format.
	///
	/// Format: magic `b"CHJC"` + version `1` + color section + BRep section.
	/// Face colors are keyed by `TopExp_Explorer` traversal index, which is
	/// stable across BRep serialization round-trips.
	///
	/// # Errors
	/// Returns [`Error::BrepReadFailed`] if the magic, version, or BRep data
	/// is invalid.
	#[cfg(feature = "color")]
	pub fn read_brep_color(reader: &mut impl Read) -> Result<Shape, Error> {
		// ① header
		let mut magic = [0u8; 4];
		reader
			.read_exact(&mut magic)
			.map_err(|_| Error::BrepReadFailed)?;
		if &magic != b"CHJC" {
			return Err(Error::BrepReadFailed);
		}
		let mut ver = [0u8; 1];
		reader
			.read_exact(&mut ver)
			.map_err(|_| Error::BrepReadFailed)?;
		if ver[0] != 1 {
			return Err(Error::BrepReadFailed);
		}

		// ② color entries
		let mut buf4 = [0u8; 4];
		reader
			.read_exact(&mut buf4)
			.map_err(|_| Error::BrepReadFailed)?;
		let color_count = u32::from_le_bytes(buf4) as usize;
		let mut entries: Vec<(u32, f32, f32, f32)> = Vec::with_capacity(color_count);
		for _ in 0..color_count {
			let mut e = [0u8; 16];
			reader
				.read_exact(&mut e)
				.map_err(|_| Error::BrepReadFailed)?;
			let idx = u32::from_le_bytes(e[0..4].try_into().unwrap());
			let r = f32::from_le_bytes(e[4..8].try_into().unwrap());
			let g = f32::from_le_bytes(e[8..12].try_into().unwrap());
			let b = f32::from_le_bytes(e[12..16].try_into().unwrap());
			entries.push((idx, r, g, b));
		}

		// ③ BRep data
		// brep_len は書き込み側の対称性のために存在するが、BRep は最終セクションなので
		// reader の残りバイトがそのまま BRep データ。直接 read_brep_bin_stream に渡す。
		let mut buf8 = [0u8; 8];
		reader
			.read_exact(&mut buf8)
			.map_err(|_| Error::BrepReadFailed)?;
		let mut rust_reader = RustReader::from_ref(reader);
		let inner = ffi::read_brep_bin_stream(&mut rust_reader);
		if inner.is_null() {
			return Err(Error::BrepReadFailed);
		}

		// ④ face index → TShapeId
		let index_to_id: Vec<TShapeId> =
			FaceIterator::new(ffi::explore_faces(&inner))
				.map(|f| f.tshape_id())
				.collect();

		// ⑤ colormap
		let colormap = entries
			.into_iter()
			.filter_map(|(idx, r, g, b)| {
				index_to_id
					.get(idx as usize)
					.map(|&id| (id, Rgb { r, g, b }))
			})
			.collect();

		Ok(Shape { inner, colormap })
	}

	/// Write this shape (with colors) to the CHJC binary format.
	///
	/// Format: magic `b"CHJC"` + version `1` + color section + BRep section.
	///
	/// # Errors
	/// Returns [`Error::BrepWriteFailed`] if writing fails.
	#[cfg(feature = "color")]
	pub fn write_brep_color(&self, writer: &mut impl Write) -> Result<(), Error> {
		// ① BRep をバッファに書き出す
		let mut brep_buf = Vec::new();
		self.write_brep_bin(&mut brep_buf)?;

		// ② TShapeId → face_index の逆引きマップ
		let id_to_index: std::collections::HashMap<TShapeId, u32> =
			FaceIterator::new(ffi::explore_faces(&self.inner))
				.enumerate()
				.map(|(i, f)| (f.tshape_id(), i as u32))
				.collect();

		// ③ colormap → (face_index, r, g, b) エントリ（決定論的出力のためソート）
		let mut entries: Vec<(u32, f32, f32, f32)> = self
			.colormap
			.iter()
			.filter_map(|(id, rgb)| {
				id_to_index.get(id).map(|&idx| (idx, rgb.r, rgb.g, rgb.b))
			})
			.collect();
		entries.sort_by_key(|e| e.0);

		// ④ 書き出し
		writer
			.write_all(b"CHJC")
			.map_err(|_| Error::BrepWriteFailed)?;
		writer
			.write_all(&[1u8])
			.map_err(|_| Error::BrepWriteFailed)?;
		writer
			.write_all(&(entries.len() as u32).to_le_bytes())
			.map_err(|_| Error::BrepWriteFailed)?;
		for (idx, r, g, b) in &entries {
			writer
				.write_all(&idx.to_le_bytes())
				.map_err(|_| Error::BrepWriteFailed)?;
			writer
				.write_all(&r.to_le_bytes())
				.map_err(|_| Error::BrepWriteFailed)?;
			writer
				.write_all(&g.to_le_bytes())
				.map_err(|_| Error::BrepWriteFailed)?;
			writer
				.write_all(&b.to_le_bytes())
				.map_err(|_| Error::BrepWriteFailed)?;
		}
		writer
			.write_all(&(brep_buf.len() as u64).to_le_bytes())
			.map_err(|_| Error::BrepWriteFailed)?;
		writer
			.write_all(&brep_buf)
			.map_err(|_| Error::BrepWriteFailed)?;
		Ok(())
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
		Ok(Shape {
			inner,
			#[cfg(feature = "color")]
			colormap: std::collections::HashMap::new(),
		})
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
		Ok(Shape {
			inner,
			#[cfg(feature = "color")]
			colormap: std::collections::HashMap::new(),
		})
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
	pub fn half_space(plane_origin: DVec3, plane_normal: DVec3) -> Shape {
		let inner = ffi::make_half_space(
			plane_origin.x,
			plane_origin.y,
			plane_origin.z,
			plane_normal.x,
			plane_normal.y,
			plane_normal.z,
		);
		Shape {
			inner,
			#[cfg(feature = "color")]
			colormap: std::collections::HashMap::new(),
		}
	}

	/// Create a box from two opposite corner points.
	///
	/// The corners are normalized internally (min/max), so the order
	/// of the points does not matter.
	pub fn box_from_corners(corner_1: DVec3, corner_2: DVec3) -> Shape {
		let inner = ffi::make_box(
			corner_1.x, corner_1.y, corner_1.z, corner_2.x, corner_2.y, corner_2.z,
		);
		Shape {
			inner,
			#[cfg(feature = "color")]
			colormap: std::collections::HashMap::new(),
		}
	}

	/// Create a cylinder.
	///
	/// - `p`: center of the base circle
	/// - `r`: radius
	/// - `dir`: axis direction
	/// - `h`: height along the axis
	pub fn cylinder(p: DVec3, r: f64, dir: DVec3, h: f64) -> Shape {
		let inner = ffi::make_cylinder(p.x, p.y, p.z, dir.x, dir.y, dir.z, r, h);
		Shape {
			inner,
			#[cfg(feature = "color")]
			colormap: std::collections::HashMap::new(),
		}
	}

	/// Create an empty compound shape.
	///
	/// Uses `TopoDS_Compound` + `BRep_Builder::MakeCompound` instead of
	/// a null shape, because null shapes cause boolean operations to fail.
	pub fn empty() -> Shape {
		let inner = ffi::make_empty();
		Shape {
			inner,
			#[cfg(feature = "color")]
			colormap: std::collections::HashMap::new(),
		}
	}

	/// Decompose this compound into its constituent solids.
	///
	/// Consumes `self` and returns a `Vec<Shape>`, one per `TopAbs_SOLID`
	/// found by `TopExp_Explorer`. The returned shapes share the same
	/// underlying B-Rep geometry via reference-counted handles (no deep copy).
	/// Returns an empty `Vec` if the shape contains no solids.
	pub fn into_solids(self) -> Vec<Shape> {
		let solids = ffi::decompose_into_solids(&self.inner);
		solids
			.iter()
			.map(|s| {
				let inner = ffi::shallow_copy(s);
				Shape {
					inner,
					#[cfg(feature = "color")]
					colormap: self.colormap.clone(),
				}
			})
			.collect()
	}

	/// Build a compound shape from a collection of shapes.
	///
	/// Uses `BRep_Builder::Add` to assemble shapes into a `TopoDS_Compound`.
	/// Only lightweight handle copies are performed (no deep copy).
	pub fn from_solids(solids: Vec<Shape>) -> Shape {
		let mut compound = ffi::make_empty();
		#[cfg(feature = "color")]
		let mut colormap = std::collections::HashMap::new();
		for s in &solids {
			ffi::compound_add(compound.pin_mut(), &s.inner);
			#[cfg(feature = "color")]
			colormap.extend(s.colormap.iter().map(|(&k, &v)| (k, v)));
		}
		Shape {
			inner: compound,
			#[cfg(feature = "color")]
			colormap,
		}
	}

	/// Create an independent deep copy of this shape.
	///
	/// Uses `BRepBuilderAPI_Copy` to create a complete copy that shares
	/// no internal `Handle<Geom_XXX>` references with the original.
	pub fn deep_copy(&self) -> Shape {
		let inner = ffi::deep_copy(&self.inner);
		#[cfg(feature = "color")]
		{
			let colormap = remap_colormap_by_order(&self.inner, &inner, &self.colormap);
			return Shape { inner, colormap };
		}
		#[cfg(not(feature = "color"))]
		Shape {
			inner,
			#[cfg(feature = "color")]
			colormap: std::collections::HashMap::new(),
		}
	}
}

// ==================== Color helpers ====================

/// Remap a colormap by matching old and new faces by traversal order.
///
/// Used for topology-preserving operations (translate, deep_copy) where
/// `BRepBuilderAPI_Transform`/`Copy` keeps faces in the same `TopExp_Explorer`
/// order.  Each old face maps 1-to-1 to the new face at the same index.
#[cfg(feature = "color")]
fn remap_colormap_by_order(
	old_inner: &ffi::TopoDS_Shape,
	new_inner: &ffi::TopoDS_Shape,
	old_colormap: &std::collections::HashMap<TShapeId, Rgb>,
) -> std::collections::HashMap<TShapeId, Rgb> {
	use crate::iterators::FaceIterator;
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

// ==================== Boolean Operations ====================

/// Merge two colormap remapping tables into a result colormap.
///
/// `from_x` is a flat array of `[post_id, src_id, ...]` pairs.
/// Looks up `src_id` in `colormap_x`; if found, inserts `post_id → color`.
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
		self.build_boolean_shape(r, other)
	}

	/// Boolean subtraction (cut) with another shape.
	///
	/// Use [`BooleanShape::is_tool_face`] to identify the cross-section faces
	/// generated at the tool boundary.
	///
	/// See [`union`](Self::union) for details on automatic deep-copy.
	pub fn subtract(&self, other: &Shape) -> Result<BooleanShape, Error> {
		let r = ffi::boolean_cut(&self.inner, &other.inner);
		if r.is_null() {
			return Err(Error::BooleanOperationFailed);
		}
		self.build_boolean_shape(r, other)
	}

	/// Boolean intersection (common) with another shape.
	///
	/// Use [`BooleanShape::is_tool_face`] to identify the cross-section faces
	/// generated at the tool boundary.
	///
	/// See [`union`](Self::union) for details on automatic deep-copy.
	pub fn intersect(&self, other: &Shape) -> Result<BooleanShape, Error> {
		let r = ffi::boolean_common(&self.inner, &other.inner);
		if r.is_null() {
			return Err(Error::BooleanOperationFailed);
		}
		self.build_boolean_shape(r, other)
	}

	fn build_boolean_shape(
		&self,
		r: cxx::UniquePtr<ffi::BooleanShape>,
		#[cfg_attr(not(feature = "color"), allow(unused_variables))]
		other: &Shape,
	) -> Result<BooleanShape, Error> {
		let from_a = ffi::boolean_shape_from_a(&r);
		let from_b = ffi::boolean_shape_from_b(&r);
		#[cfg(feature = "color")]
		let colormap = merge_colormaps(&from_a, &from_b, &self.colormap, &other.colormap);
		Ok(BooleanShape {
			shape: Shape {
				inner: ffi::boolean_shape_shape(&r),
				#[cfg(feature = "color")]
				colormap,
			},
			from_a,
			from_b,
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
		#[cfg(feature = "color")]
		{
			let r = ffi::clean_shape_full(&self.inner);
			if r.is_null() {
				return Err(Error::CleanFailed);
			}
			let inner = ffi::clean_shape_get(&r);
			if inner.is_null() {
				return Err(Error::CleanFailed);
			}
			let mapping = ffi::clean_shape_mapping(&r);
			let mut colormap = std::collections::HashMap::new();
			for pair in mapping.chunks(2) {
				let new_id = TShapeId(pair[0]);
				let old_id = TShapeId(pair[1]);
				if let Some(&color) = self.colormap.get(&old_id) {
					// First-found wins when multiple old faces merge into one.
					colormap.entry(new_id).or_insert(color);
				}
			}
			return Ok(Shape { inner, colormap });
		}
		#[cfg(not(feature = "color"))]
		{
			let inner = ffi::clean_shape(&self.inner);
			if inner.is_null() {
				return Err(Error::CleanFailed);
			}
			Ok(Shape {
			inner,
			#[cfg(feature = "color")]
			colormap: std::collections::HashMap::new(),
		})
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
		let colormap = remap_colormap_by_order(&self.inner, &inner, &self.colormap);
		Shape {
			inner,
			#[cfg(feature = "color")]
			colormap,
		}
	}

	/// Create a new shape rotated around an axis.
	///
	/// Uses `BRepBuilderAPI_Transform` with `gp_Trsf::SetRotation`.
	///
	/// - `axis_origin`: a point on the rotation axis
	/// - `axis_direction`: direction of the rotation axis
	/// - `angle`: rotation angle in radians
	pub fn rotated(&self, axis_origin: DVec3, axis_direction: DVec3, angle: f64) -> Shape {
		let inner = ffi::rotate_shape(
			&self.inner,
			axis_origin.x, axis_origin.y, axis_origin.z,
			axis_direction.x, axis_direction.y, axis_direction.z,
			angle,
		);
		#[cfg(feature = "color")]
		let colormap = remap_colormap_by_order(&self.inner, &inner, &self.colormap);
		Shape {
			inner,
			#[cfg(feature = "color")]
			colormap,
		}
	}

	/// Create a new shape uniformly scaled around a center point.
	///
	/// Uses `BRepBuilderAPI_Transform` with `gp_Trsf::SetScale`.
	/// Only uniform scaling (same factor for all axes) is supported.
	///
	/// - `center`: center of scaling
	/// - `factor`: scale factor
	pub fn scaled(&self, center: DVec3, factor: f64) -> Shape {
		let inner = ffi::scale_shape(
			&self.inner,
			center.x, center.y, center.z,
			factor,
		);
		#[cfg(feature = "color")]
		let colormap = remap_colormap_by_order(&self.inner, &inner, &self.colormap);
		Shape {
			inner,
			#[cfg(feature = "color")]
			colormap,
		}
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
		let face_ids = data.face_tshape_ids;

		Ok(Mesh {
			vertices,
			uvs,
			normals,
			indices,
			face_ids,
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

	/// Check if a point is inside this solid shape.
	///
	/// Uses `BRepClass3d_SolidClassifier` with tolerance `1e-6`.
	/// Returns `true` only for points strictly inside (`TopAbs_IN`),
	/// not on the boundary. Designed for solid shapes; behavior is
	/// undefined for open shells, faces, or edges.
	pub fn contains(&self, point: DVec3) -> bool {
		ffi::shape_contains_point(&self.inner, point.x, point.y, point.z)
	}

	/// Compute the volume of this shape.
	///
	/// Uses `BRepGProp::VolumeProperties`. Returns 0 for non-solid shapes
	/// (faces, edges, compounds without volume). May return a negative value
	/// if the shape orientation is reversed.
	pub fn volume(&self) -> f64 {
		ffi::shape_volume(&self.inner)
	}

	/// Assign the same color to every face in this shape.
	///
	/// Collects all face [`TShapeId`]s first to avoid a borrow conflict
	/// between the face iterator and the mutable `colormap`.
	#[cfg(feature = "color")]
	pub fn paint(&mut self, color: Rgb) {
		let ids: Vec<TShapeId> = self.faces().map(|f| f.tshape_id()).collect();
		for id in ids {
			self.colormap.insert(id, color);
		}
	}
}

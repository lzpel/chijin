use super::compound::Compound;
use super::ffi;
use super::iterators::FaceIterator;
use super::solid::Solid;
use super::stream::{RustReader, RustWriter};
use crate::common::error::Error;
use crate::traits::IoModule;
use std::io::{Read, Write};

#[cfg(feature = "color")]
use crate::common::color::Color;

/// I/O operations backed by OpenCASCADE.
pub struct Io;

// ==================== Color trailer ====================

#[cfg(feature = "color")]
const COLOR_TRAILER_MAGIC: &[u8; 4] = b"CDCL";

#[cfg(feature = "color")]
fn strip_color_trailer(buf: &[u8]) -> (std::collections::HashMap<u64, Color>, usize) {
	if buf.len() < 8 || &buf[buf.len() - 4..] != COLOR_TRAILER_MAGIC {
		return (std::collections::HashMap::new(), buf.len());
	}
	let entry_count = u32::from_le_bytes(buf[buf.len() - 8..buf.len() - 4].try_into().unwrap()) as usize;
	let trailer_size = 8 + entry_count * 16;
	if buf.len() < trailer_size {
		return (std::collections::HashMap::new(), buf.len());
	}
	let brep_len = buf.len() - trailer_size;
	let entries_start = brep_len;
	let mut colormap = std::collections::HashMap::new();
	for i in 0..entry_count {
		let off = entries_start + i * 16;
		let idx = u32::from_le_bytes(buf[off..off + 4].try_into().unwrap());
		let r = f32::from_le_bytes(buf[off + 4..off + 8].try_into().unwrap());
		let g = f32::from_le_bytes(buf[off + 8..off + 12].try_into().unwrap());
		let b = f32::from_le_bytes(buf[off + 12..off + 16].try_into().unwrap());
		colormap.insert(idx as u64, Color { r, g, b });
	}
	(colormap, brep_len)
}

#[cfg(feature = "color")]
fn resolve_color_trailer(inner: &ffi::TopoDS_Shape, index_colormap: &std::collections::HashMap<u64, Color>) -> std::collections::HashMap<u64, Color> {
	let index_to_id: Vec<u64> = FaceIterator::new(ffi::explore_faces(inner)).map(|f| f.tshape_id()).collect();
	index_colormap.iter().filter_map(|(&idx, &color)| index_to_id.get(idx as usize).map(|&id| (id, color))).collect()
}

#[cfg(feature = "color")]
fn write_color_trailer<W: Write>(compound: &Compound, writer: &mut W) -> Result<(), Error> {
	let colormap = compound.colormap();
	if colormap.is_empty() {
		return Ok(());
	}
	let id_to_index: std::collections::HashMap<u64, u32> = FaceIterator::new(ffi::explore_faces(compound.inner())).enumerate().map(|(i, f)| (f.tshape_id(), i as u32)).collect();
	let mut entries: Vec<(u32, f32, f32, f32)> = colormap.iter().filter_map(|(id, rgb)| id_to_index.get(id).map(|&idx| (idx, rgb.r, rgb.g, rgb.b))).collect();
	entries.sort_by_key(|e| e.0);

	for (idx, r, g, b) in &entries {
		writer.write_all(&idx.to_le_bytes()).map_err(|_| Error::BrepWriteFailed)?;
		writer.write_all(&r.to_le_bytes()).map_err(|_| Error::BrepWriteFailed)?;
		writer.write_all(&g.to_le_bytes()).map_err(|_| Error::BrepWriteFailed)?;
		writer.write_all(&b.to_le_bytes()).map_err(|_| Error::BrepWriteFailed)?;
	}
	writer.write_all(&(entries.len() as u32).to_le_bytes()).map_err(|_| Error::BrepWriteFailed)?;
	writer.write_all(COLOR_TRAILER_MAGIC).map_err(|_| Error::BrepWriteFailed)?;
	Ok(())
}

// ==================== IoTrait implementation ====================

impl IoModule for Io {
	fn read_step<R: Read>(reader: &mut R) -> Result<Vec<Solid>, Error> {
		#[cfg(feature = "color")]
		{
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
				colormap.insert(ids[i], Color { r: r[i], g: g[i], b: b[i] });
			}
			Ok(Compound::from_raw(inner, colormap).decompose())
		}
		#[cfg(not(feature = "color"))]
		{
			let mut rust_reader = RustReader::from_ref(reader);
			let inner = ffi::read_step_stream(&mut rust_reader);
			if inner.is_null() {
				return Err(Error::StepReadFailed);
			}
			Ok(Compound::from_raw(inner).decompose())
		}
	}

	/// Read a shape from a BRep binary format stream.
	///
	/// With the `color` feature enabled, a color trailer (if present) at the end
	/// of the data is parsed to reconstruct the per-face colormap.
	fn read_brep_binary<R: Read>(reader: &mut R) -> Result<Vec<Solid>, Error> {
		#[cfg(feature = "color")]
		{
			let mut buf = Vec::new();
			reader.read_to_end(&mut buf).map_err(|_| Error::BrepReadFailed)?;
			let (index_colormap, brep_len) = strip_color_trailer(&buf);
			let mut cursor = std::io::Cursor::new(&buf[..brep_len]);
			let mut rust_reader = RustReader::from_ref(&mut cursor);
			let inner = ffi::read_brep_bin_stream(&mut rust_reader);
			if inner.is_null() {
				return Err(Error::BrepReadFailed);
			}
			let colormap = resolve_color_trailer(&inner, &index_colormap);
			Ok(Compound::from_raw(inner, colormap).decompose())
		}
		#[cfg(not(feature = "color"))]
		{
			let mut rust_reader = RustReader::from_ref(reader);
			let inner = ffi::read_brep_bin_stream(&mut rust_reader);
			if inner.is_null() {
				return Err(Error::BrepReadFailed);
			}
			Ok(Compound::from_raw(inner).decompose())
		}
	}

	/// Read a shape from a BRep text format stream.
	///
	/// With the `color` feature enabled, a color trailer (if present) at the end
	/// of the data is parsed to reconstruct the per-face colormap.
	fn read_brep_text<R: Read>(reader: &mut R) -> Result<Vec<Solid>, Error> {
		#[cfg(feature = "color")]
		{
			let mut buf = Vec::new();
			reader.read_to_end(&mut buf).map_err(|_| Error::BrepReadFailed)?;
			let (index_colormap, brep_len) = strip_color_trailer(&buf);
			let mut cursor = std::io::Cursor::new(&buf[..brep_len]);
			let mut rust_reader = RustReader::from_ref(&mut cursor);
			let inner = ffi::read_brep_text_stream(&mut rust_reader);
			if inner.is_null() {
				return Err(Error::BrepReadFailed);
			}
			let colormap = resolve_color_trailer(&inner, &index_colormap);
			Ok(Compound::from_raw(inner, colormap).decompose())
		}
		#[cfg(not(feature = "color"))]
		{
			let mut rust_reader = RustReader::from_ref(reader);
			let inner = ffi::read_brep_text_stream(&mut rust_reader);
			if inner.is_null() {
				return Err(Error::BrepReadFailed);
			}
			Ok(Compound::from_raw(inner).decompose())
		}
	}

	// ==================== Write ====================

	/// Write a shape to a STEP format stream.
	///
	/// With the `color` feature enabled, face colors are automatically embedded
	/// in the STEP file (XDE / AP214 styled items).
	fn write_step<'a, W: Write>(solids: impl IntoIterator<Item = &'a Solid>, writer: &mut W) -> Result<(), Error> {
		let compound = Compound::new(solids);
		#[cfg(feature = "color")]
		{
			let colormap = compound.colormap();
			let ids: Vec<u64> = colormap.keys().copied().collect();
			let r: Vec<f32> = ids.iter().map(|&id| colormap[&id].r).collect();
			let g: Vec<f32> = ids.iter().map(|&id| colormap[&id].g).collect();
			let b: Vec<f32> = ids.iter().map(|&id| colormap[&id].b).collect();
			let mut rust_writer = RustWriter::from_ref(writer);
			if ffi::write_step_color_stream(compound.inner(), &ids, &r, &g, &b, &mut rust_writer) {
				Ok(())
			} else {
				Err(Error::StepWriteFailed)
			}
		}
		#[cfg(not(feature = "color"))]
		{
			let mut rust_writer = RustWriter::from_ref(writer);
			if ffi::write_step_stream(compound.inner(), &mut rust_writer) {
				Ok(())
			} else {
				Err(Error::StepWriteFailed)
			}
		}
	}

	/// Write a shape to a BRep binary format stream.
	///
	/// With the `color` feature enabled, a color trailer is appended after the
	/// BRep data so that face colors survive the round-trip.
	fn write_brep_binary<'a, W: Write>(solids: impl IntoIterator<Item = &'a Solid>, writer: &mut W) -> Result<(), Error> {
		let compound = Compound::new(solids);
		let mut rust_writer = RustWriter::from_ref(writer);
		if !ffi::write_brep_bin_stream(compound.inner(), &mut rust_writer) {
			return Err(Error::BrepWriteFailed);
		}
		#[cfg(feature = "color")]
		write_color_trailer(&compound, writer)?;
		Ok(())
	}

	/// Write a shape to a BRep text format stream.
	///
	/// With the `color` feature enabled, a color trailer is appended after the
	/// BRep data so that face colors survive the round-trip.
	fn write_brep_text<'a, W: Write>(solids: impl IntoIterator<Item = &'a Solid>, writer: &mut W) -> Result<(), Error> {
		let compound = Compound::new(solids);
		let mut rust_writer = RustWriter::from_ref(writer);
		if !ffi::write_brep_text_stream(compound.inner(), &mut rust_writer) {
			return Err(Error::BrepWriteFailed);
		}
		#[cfg(feature = "color")]
		write_color_trailer(&compound, writer)?;
		Ok(())
	}

	fn mesh<'a>(solids: impl IntoIterator<Item = &'a Solid>, tolerance: f64) -> Result<crate::common::mesh::Mesh, Error> {
		use crate::common::mesh::{EdgeData, Mesh};
		use glam::{DVec2, DVec3};

		let compound = Compound::new(solids);
		let data = ffi::mesh_shape(compound.inner(), tolerance);
		if !data.success {
			return Err(Error::TriangulationFailed);
		}
		let vertex_count = data.vertices.len() / 3;
		let vertices: Vec<DVec3> = (0..vertex_count).map(|i| DVec3::new(data.vertices[i * 3], data.vertices[i * 3 + 1], data.vertices[i * 3 + 2])).collect();
		let uvs: Vec<DVec2> = (0..vertex_count).map(|i| DVec2::new(data.uvs[i * 2], data.uvs[i * 2 + 1])).collect();
		let normals: Vec<DVec3> = (0..vertex_count).map(|i| DVec3::new(data.normals[i * 3], data.normals[i * 3 + 1], data.normals[i * 3 + 2])).collect();
		let indices: Vec<usize> = data.indices.iter().map(|&i| i as usize).collect();
		let face_ids = data.face_tshape_ids;

		#[cfg(feature = "color")]
		let colormap = {
			let mut map = std::collections::HashMap::new();
			for &fid in &face_ids {
				if let Some(&color) = compound.colormap().get(&fid) {
					map.insert(fid, color);
				}
			}
			map
		};

		Ok(Mesh {
			vertices,
			uvs,
			normals,
			indices,
			face_ids,
			#[cfg(feature = "color")]
			colormap,
			edges: EdgeData::default(),
		})
	}

	fn write_svg<'a, W: Write>(solids: impl IntoIterator<Item = &'a Solid>, direction: glam::DVec3, tolerance: f64, writer: &mut W) -> Result<(), Error> {
		let combined = Self::mesh(solids, tolerance)?;
		writer.write_all(combined.to_svg(direction).as_bytes()).map_err(|_| Error::SvgExportFailed)
	}
} // impl ioTrait for io

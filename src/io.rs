use crate::error::Error;
use crate::ffi;
use crate::iterators::FaceIterator;
use crate::shape::{to_compound, decompose};
use crate::solid::Solid;
use crate::stream::{RustReader, RustWriter};
use std::io::{Read, Write};

#[cfg(feature = "color")]
use crate::shape::{merge_all_colormaps, Rgb, TShapeId};

// ==================== Read ====================

/// Read a shape from a STEP format stream.
pub fn read_step<R: Read>(reader: &mut R) -> Result<Vec<Solid>, Error> {
    let mut rust_reader = RustReader::from_ref(reader);
    let inner = ffi::read_step_stream(&mut rust_reader);
    if inner.is_null() {
        return Err(Error::StepReadFailed);
    }
    Ok(decompose(
        &inner,
        #[cfg(feature = "color")]
        &std::collections::HashMap::new(),
    ))
}

/// Read a shape from a BRep binary format stream.
pub fn read_brep_bin<R: Read>(reader: &mut R) -> Result<Vec<Solid>, Error> {
    let mut rust_reader = RustReader::from_ref(reader);
    let inner = ffi::read_brep_bin_stream(&mut rust_reader);
    if inner.is_null() {
        return Err(Error::BrepReadFailed);
    }
    Ok(decompose(
        &inner,
        #[cfg(feature = "color")]
        &std::collections::HashMap::new(),
    ))
}

/// Read a shape from a BRep text format stream.
pub fn read_brep_text<R: Read>(reader: &mut R) -> Result<Vec<Solid>, Error> {
    let mut rust_reader = RustReader::from_ref(reader);
    let inner = ffi::read_brep_text_stream(&mut rust_reader);
    if inner.is_null() {
        return Err(Error::BrepReadFailed);
    }
    Ok(decompose(
        &inner,
        #[cfg(feature = "color")]
        &std::collections::HashMap::new(),
    ))
}

/// Read a STEP file and populate colormaps with face colors.
#[cfg(feature = "color")]
pub fn read_step_with_colors<R: Read>(reader: &mut R) -> Result<Vec<Solid>, Error> {
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
    Ok(decompose(&inner, &colormap))
}

/// Read a shape (with colors) from the CHJC binary format.
#[cfg(feature = "color")]
pub fn read_brep_color<R: Read>(reader: &mut R) -> Result<Vec<Solid>, Error> {
    let mut magic = [0u8; 4];
    reader.read_exact(&mut magic).map_err(|_| Error::BrepReadFailed)?;
    if &magic != b"CHJC" {
        return Err(Error::BrepReadFailed);
    }
    let mut ver = [0u8; 1];
    reader.read_exact(&mut ver).map_err(|_| Error::BrepReadFailed)?;
    if ver[0] != 1 {
        return Err(Error::BrepReadFailed);
    }

    let mut buf4 = [0u8; 4];
    reader.read_exact(&mut buf4).map_err(|_| Error::BrepReadFailed)?;
    let color_count = u32::from_le_bytes(buf4) as usize;
    let mut entries: Vec<(u32, f32, f32, f32)> = Vec::with_capacity(color_count);
    for _ in 0..color_count {
        let mut e = [0u8; 16];
        reader.read_exact(&mut e).map_err(|_| Error::BrepReadFailed)?;
        let idx = u32::from_le_bytes(e[0..4].try_into().unwrap());
        let r = f32::from_le_bytes(e[4..8].try_into().unwrap());
        let g = f32::from_le_bytes(e[8..12].try_into().unwrap());
        let b = f32::from_le_bytes(e[12..16].try_into().unwrap());
        entries.push((idx, r, g, b));
    }

    let mut buf8 = [0u8; 8];
    reader.read_exact(&mut buf8).map_err(|_| Error::BrepReadFailed)?;
    let mut rust_reader = RustReader::from_ref(reader);
    let inner = ffi::read_brep_bin_stream(&mut rust_reader);
    if inner.is_null() {
        return Err(Error::BrepReadFailed);
    }

    let index_to_id: Vec<TShapeId> = FaceIterator::new(ffi::explore_faces(&inner))
        .map(|f| f.tshape_id())
        .collect();

    let colormap = entries
        .into_iter()
        .filter_map(|(idx, r, g, b)| {
            index_to_id.get(idx as usize).map(|&id| (id, Rgb { r, g, b }))
        })
        .collect();

    Ok(decompose(&inner, &colormap))
}

// ==================== Write ====================

/// Write a shape to a STEP format stream.
pub fn write_step<W: Write>(solids: &[Solid], writer: &mut W) -> Result<(), Error> {
    let compound = to_compound(solids);
    let mut rust_writer = RustWriter::from_ref(writer);
    if ffi::write_step_stream(&compound, &mut rust_writer) {
        Ok(())
    } else {
        Err(Error::StepWriteFailed)
    }
}

/// Write a shape to a BRep binary format stream.
pub fn write_brep_bin<W: Write>(solids: &[Solid], writer: &mut W) -> Result<(), Error> {
    let compound = to_compound(solids);
    let mut rust_writer = RustWriter::from_ref(writer);
    if ffi::write_brep_bin_stream(&compound, &mut rust_writer) {
        Ok(())
    } else {
        Err(Error::BrepWriteFailed)
    }
}

/// Write a shape to a BRep text format stream.
pub fn write_brep_text<W: Write>(solids: &[Solid], writer: &mut W) -> Result<(), Error> {
    let compound = to_compound(solids);
    let mut rust_writer = RustWriter::from_ref(writer);
    if ffi::write_brep_text_stream(&compound, &mut rust_writer) {
        Ok(())
    } else {
        Err(Error::BrepWriteFailed)
    }
}

/// Write a shape with face colors to a STEP format stream.
#[cfg(feature = "color")]
pub fn write_step_with_colors<W: Write>(solids: &[Solid], writer: &mut W) -> Result<(), Error> {
    let compound = to_compound(solids);
    let colormap = merge_all_colormaps(solids);
    let ids: Vec<u64> = colormap.keys().map(|k| k.0).collect();
    let r: Vec<f32> = ids.iter().map(|&id| colormap[&TShapeId(id)].r).collect();
    let g: Vec<f32> = ids.iter().map(|&id| colormap[&TShapeId(id)].g).collect();
    let b: Vec<f32> = ids.iter().map(|&id| colormap[&TShapeId(id)].b).collect();
    let mut rust_writer = RustWriter::from_ref(writer);
    if ffi::write_step_color_stream(&compound, &ids, &r, &g, &b, &mut rust_writer) {
        Ok(())
    } else {
        Err(Error::StepWriteFailed)
    }
}

/// Write a shape with face colors to the CHJC binary format.
#[cfg(feature = "color")]
pub fn write_brep_color<W: Write>(solids: &[Solid], writer: &mut W) -> Result<(), Error> {
    let compound = to_compound(solids);
    let colormap = merge_all_colormaps(solids);

    let mut brep_buf = Vec::new();
    {
        let mut rust_writer = RustWriter::from_ref(&mut brep_buf);
        if !ffi::write_brep_bin_stream(&compound, &mut rust_writer) {
            return Err(Error::BrepWriteFailed);
        }
    }

    let id_to_index: std::collections::HashMap<TShapeId, u32> =
        FaceIterator::new(ffi::explore_faces(&compound))
            .enumerate()
            .map(|(i, f)| (f.tshape_id(), i as u32))
            .collect();

    let mut entries: Vec<(u32, f32, f32, f32)> = colormap
        .iter()
        .filter_map(|(id, rgb)| {
            id_to_index.get(id).map(|&idx| (idx, rgb.r, rgb.g, rgb.b))
        })
        .collect();
    entries.sort_by_key(|e| e.0);

    writer.write_all(b"CHJC").map_err(|_| Error::BrepWriteFailed)?;
    writer.write_all(&[1u8]).map_err(|_| Error::BrepWriteFailed)?;
    writer.write_all(&(entries.len() as u32).to_le_bytes()).map_err(|_| Error::BrepWriteFailed)?;
    for (idx, r, g, b) in &entries {
        writer.write_all(&idx.to_le_bytes()).map_err(|_| Error::BrepWriteFailed)?;
        writer.write_all(&r.to_le_bytes()).map_err(|_| Error::BrepWriteFailed)?;
        writer.write_all(&g.to_le_bytes()).map_err(|_| Error::BrepWriteFailed)?;
        writer.write_all(&b.to_le_bytes()).map_err(|_| Error::BrepWriteFailed)?;
    }
    writer.write_all(&(brep_buf.len() as u64).to_le_bytes()).map_err(|_| Error::BrepWriteFailed)?;
    writer.write_all(&brep_buf).map_err(|_| Error::BrepWriteFailed)?;
    Ok(())
}

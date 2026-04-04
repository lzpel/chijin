use std::io::{Read, Write};

/// Wrapper around `dyn Read` passed to C++ as an opaque extern Rust type.
///
/// C++ calls `rust_reader_read()` to pull bytes from the Rust reader,
/// feeding them into a `std::streambuf` subclass that OCC reads from.
///
/// # Safety
/// The lifetime is erased internally. The caller must ensure the reader
/// outlives the FFI call (which is always the case since C++ calls are
/// synchronous and blocking).
pub struct RustReader {
	inner: *mut dyn Read,
}

impl RustReader {
	/// Create a new RustReader wrapping the given reader.
	///
	/// # Safety
	/// The caller must ensure that the resulting `RustReader` is not used
	/// after `reader` is dropped. In practice, this is guaranteed because
	/// the C++ FFI call is synchronous.
	pub fn from_ref<'a>(reader: &'a mut (dyn Read + 'a)) -> Self {
		// SAFETY: Caller must ensure `reader` outlives this RustReader.
		// The `'static` bound is required by the raw pointer type, so we
		// use transmute to erase the lifetime (lifetimes are compile-time only).
		RustReader {
			inner: unsafe {
				std::mem::transmute::<*mut (dyn Read + 'a), *mut (dyn Read + 'static)>(
					reader as *mut (dyn Read + 'a),
				)
			},
		}
	}
}

/// Wrapper around `dyn Write` passed to C++ as an opaque extern Rust type.
///
/// C++ calls `rust_writer_write()` to push bytes into the Rust writer,
/// receiving them from a `std::streambuf` subclass that OCC writes to.
pub struct RustWriter {
	inner: *mut dyn Write,
}

impl RustWriter {
	/// Create a new RustWriter wrapping the given writer.
	///
	/// # Safety
	/// Same as `RustReader::from_ref`.
	pub fn from_ref<'a>(writer: &'a mut (dyn Write + 'a)) -> Self {
		// SAFETY: Caller must ensure `writer` outlives this RustWriter.
		// See RustReader::from_ref for the same rationale.
		RustWriter {
			inner: unsafe {
				std::mem::transmute::<*mut (dyn Write + 'a), *mut (dyn Write + 'static)>(
					writer as *mut (dyn Write + 'a),
				)
			},
		}
	}
}

/// FFI callback: read up to `buf.len()` bytes from the RustReader.
/// Returns the number of bytes actually read (0 = EOF).
pub fn rust_reader_read(reader: &mut RustReader, buf: &mut [u8]) -> usize {
	unsafe { (*reader.inner).read(buf).unwrap_or(0) }
}

/// FFI callback: write bytes into the RustWriter.
/// Returns the number of bytes actually written.
pub fn rust_writer_write(writer: &mut RustWriter, buf: &[u8]) -> usize {
	unsafe { (*writer.inner).write(buf).unwrap_or(0) }
}

/// FFI callback: flush the RustWriter.
pub fn rust_writer_flush(writer: &mut RustWriter) -> bool {
	unsafe { (*writer.inner).flush().is_ok() }
}

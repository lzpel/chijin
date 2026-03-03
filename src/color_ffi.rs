//! Color FFI bridge — compiled only when `feature = "color"` is enabled.
//!
//! This module provides the cxx bridge declarations for the ColorMap C++ class
//! and all color-relay operations. The C++ implementations are guarded by
//! `#ifdef CHIJIN_COLOR` in wrapper.cpp.

#[cxx::bridge(namespace = "chijin")]
mod color_ffi_bridge {
	unsafe extern "C++" {
		include!("chijin/cpp/wrapper.h");

		// Re-declare types from the main bridge that we reference
		type TopoDS_Shape = crate::ffi::TopoDS_Shape;
		type TopoDS_Face = crate::ffi::TopoDS_Face;

		// ==================== Color Map ====================

		type ColorMap;
		type BooleanShapeColored;

		fn colormap_new() -> UniquePtr<ColorMap>;
		fn colormap_set(map: Pin<&mut ColorMap>, face: &TopoDS_Face, r: u8, g: u8, b: u8);
		fn colormap_get(
			map: &ColorMap,
			face: &TopoDS_Face,
			r: &mut u8,
			g: &mut u8,
			b: &mut u8,
		) -> bool;
		fn colormap_size(map: &ColorMap) -> i32;

		// Color-relay boolean operations
		fn boolean_fuse_colored(
			a: &TopoDS_Shape,
			a_colors: &ColorMap,
			b: &TopoDS_Shape,
			b_colors: &ColorMap,
		) -> UniquePtr<BooleanShapeColored>;
		fn boolean_cut_colored(
			a: &TopoDS_Shape,
			a_colors: &ColorMap,
			b: &TopoDS_Shape,
			b_colors: &ColorMap,
		) -> UniquePtr<BooleanShapeColored>;
		fn boolean_common_colored(
			a: &TopoDS_Shape,
			a_colors: &ColorMap,
			b: &TopoDS_Shape,
			b_colors: &ColorMap,
		) -> UniquePtr<BooleanShapeColored>;

		// BooleanShapeColored accessors
		fn colored_result_shape(r: &BooleanShapeColored) -> UniquePtr<TopoDS_Shape>;
		fn colored_result_new_faces(r: &BooleanShapeColored) -> UniquePtr<TopoDS_Shape>;
		fn colored_result_shape_colors(r: &BooleanShapeColored) -> UniquePtr<ColorMap>;
		fn colored_result_new_faces_colors(r: &BooleanShapeColored) -> UniquePtr<ColorMap>;

		// Color-relay clean
		fn clean_shape_colored(
			shape: &TopoDS_Shape,
			in_colors: &ColorMap,
			out_colors: Pin<&mut ColorMap>,
		) -> UniquePtr<TopoDS_Shape>;

		// Color remap after deep_copy
		fn remap_colors_after_copy(
			before_copy: &TopoDS_Shape,
			after_copy: &TopoDS_Shape,
			src: &ColorMap,
		) -> UniquePtr<ColorMap>;

		// ==================== XDE STEP colored I/O ====================

		/// Read a STEP byte slice with XDE color support.
		/// Face colors found in the file are written into `out_colors`.
		/// Using a byte slice avoids RustReader type sharing across cxx bridges.
		fn read_step_colored_from_slice(
			data: &[u8],
			out_colors: Pin<&mut ColorMap>,
		) -> UniquePtr<TopoDS_Shape>;

		/// Write a shape to STEP bytes with face colors via XDE.
		/// Returns an empty Vec on failure.
		fn write_step_colored_to_vec(
			shape: &TopoDS_Shape,
			colors: &ColorMap,
		) -> Vec<u8>;
	}
}

pub use color_ffi_bridge::*;

unsafe impl Send for ColorMap {}
unsafe impl Send for BooleanShapeColored {}

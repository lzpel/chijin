#pragma once

#include "rust/cxx.h"

// Types used directly in function signatures — keep minimal so that
// the cxx-generated bridge objects do not compile heavy OCCT headers.
#include <TopoDS_Shape.hxx>
#include <TopoDS_Face.hxx>
#include <TopoDS_Edge.hxx>
#include <TopExp_Explorer.hxx>

#include <cstdint>
#include <streambuf>
#include <memory>
#include <vector>

namespace cadrum {

// Type aliases to bring OCCT global types into cadrum namespace.
// Required because the cxx bridge uses namespace = "cadrum".
using TopoDS_Shape = ::TopoDS_Shape;
using TopoDS_Face = ::TopoDS_Face;
using TopoDS_Edge = ::TopoDS_Edge;
using TopExp_Explorer = ::TopExp_Explorer;

// Forward-declare the Rust opaque types (defined by cxx in ffi.rs.h)
struct RustReader;
struct RustWriter;

// Forward-declare shared structs (defined by cxx in ffi.rs.h)
struct MeshData;
struct ApproxPoints;
struct SvgEdgeData;

// ==================== Streambuf bridges ====================

// std::streambuf subclass that reads from a Rust `dyn Read` via FFI callback
class RustReadStreambuf : public std::streambuf {
public:
    explicit RustReadStreambuf(RustReader& reader) : reader_(reader) {}

protected:
    int_type underflow() override;

private:
    RustReader& reader_;
    char buf_[8192];
};

// std::streambuf subclass that writes to a Rust `dyn Write` via FFI callback
class RustWriteStreambuf : public std::streambuf {
public:
    explicit RustWriteStreambuf(RustWriter& writer) : writer_(writer) {}

    ~RustWriteStreambuf() override {
        sync();
    }

protected:
    int_type overflow(int_type ch) override;
    std::streamsize xsputn(const char* s, std::streamsize count) override;
    int sync() override;

private:
    bool flush_buf();

    RustWriter& writer_;
    char buf_[8192];
    size_t pos_ = 0;
};

// ==================== Shape I/O (streambuf callback) ====================

std::unique_ptr<TopoDS_Shape> read_step_stream(RustReader& reader);
std::unique_ptr<TopoDS_Shape> read_brep_bin_stream(RustReader& reader);
bool write_brep_bin_stream(const TopoDS_Shape& shape, RustWriter& writer);
std::unique_ptr<TopoDS_Shape> read_brep_text_stream(RustReader& reader);
bool write_brep_text_stream(const TopoDS_Shape& shape, RustWriter& writer);
bool write_step_stream(const TopoDS_Shape& shape, RustWriter& writer);

// ==================== Shape Constructors ====================

std::unique_ptr<TopoDS_Shape> make_half_space(
    double ox, double oy, double oz,
    double nx, double ny, double nz);

std::unique_ptr<TopoDS_Shape> make_box(
    double x1, double y1, double z1,
    double x2, double y2, double z2);

std::unique_ptr<TopoDS_Shape> make_cylinder(
    double px, double py, double pz,
    double dx, double dy, double dz,
    double radius, double height);

std::unique_ptr<TopoDS_Shape> make_empty();
std::unique_ptr<TopoDS_Shape> deep_copy(const TopoDS_Shape& shape);
std::unique_ptr<TopoDS_Shape> shallow_copy(const TopoDS_Shape& shape);

// ==================== Boolean Operations ====================

/// Result of a boolean operation.
///
/// from_a / from_b encode face-origin pairs as flat arrays:
///   [post_copy_tshape_id, source_tshape_id, ...]
/// Used to remap colormaps and derive new_face_ids on the Rust side.
class BooleanShape {
public:
    TopoDS_Shape shape;
    std::vector<uint64_t> from_a;  // pairs: [post_id, src_a_id, ...]
    std::vector<uint64_t> from_b;  // pairs: [post_id, src_b_id, ...]
};

std::unique_ptr<BooleanShape> boolean_fuse(
    const TopoDS_Shape& a, const TopoDS_Shape& b);
std::unique_ptr<BooleanShape> boolean_cut(
    const TopoDS_Shape& a, const TopoDS_Shape& b);
std::unique_ptr<BooleanShape> boolean_common(
    const TopoDS_Shape& a, const TopoDS_Shape& b);

std::unique_ptr<TopoDS_Shape> boolean_shape_shape(const BooleanShape& r);
rust::Vec<uint64_t> boolean_shape_from_a(const BooleanShape& r);
rust::Vec<uint64_t> boolean_shape_from_b(const BooleanShape& r);

// ==================== Shape Methods ====================

std::unique_ptr<TopoDS_Shape> clean_shape(const TopoDS_Shape& shape);
std::unique_ptr<TopoDS_Shape> translate_shape(
    const TopoDS_Shape& shape, double tx, double ty, double tz);
std::unique_ptr<TopoDS_Shape> rotate_shape(
    const TopoDS_Shape& shape,
    double ox, double oy, double oz,
    double dx, double dy, double dz,
    double angle);
std::unique_ptr<TopoDS_Shape> scale_shape(
    const TopoDS_Shape& shape,
    double cx, double cy, double cz,
    double factor);
std::unique_ptr<TopoDS_Shape> mirror_shape(
    const TopoDS_Shape& shape,
    double ox, double oy, double oz,
    double nx, double ny, double nz);
bool shape_is_null(const TopoDS_Shape& shape);
bool shape_is_solid(const TopoDS_Shape& shape);
uint32_t shape_shell_count(const TopoDS_Shape& shape);
double shape_volume(const TopoDS_Shape& shape);
bool shape_contains_point(const TopoDS_Shape& shape, double x, double y, double z);
uint64_t shape_tshape_id(const TopoDS_Shape& shape);

// ==================== Compound Decompose/Compose ====================

std::unique_ptr<std::vector<TopoDS_Shape>> decompose_into_solids(const TopoDS_Shape& shape);
void compound_add(TopoDS_Shape& compound, const TopoDS_Shape& child);

// ==================== Meshing ====================

MeshData mesh_shape(const TopoDS_Shape& shape, double tolerance);

// ==================== Explorer / Iterators ====================

std::unique_ptr<TopExp_Explorer> explore_faces(const TopoDS_Shape& shape);
std::unique_ptr<TopExp_Explorer> explore_edges(const TopoDS_Shape& shape);
bool explorer_more(const TopExp_Explorer& explorer);
void explorer_next(TopExp_Explorer& explorer);
std::unique_ptr<TopoDS_Face> explorer_current_face(const TopExp_Explorer& explorer);
std::unique_ptr<TopoDS_Edge> explorer_current_edge(const TopExp_Explorer& explorer);

// ==================== Face Methods ====================

void face_center_of_mass(const TopoDS_Face& face,
    double& cx, double& cy, double& cz);
void face_normal_at_center(const TopoDS_Face& face,
    double& nx, double& ny, double& nz);
std::unique_ptr<TopoDS_Face> face_from_polygon(rust::Slice<const double> coords);
std::unique_ptr<TopoDS_Shape> face_extrude(const TopoDS_Face& face,
    double dx, double dy, double dz);
std::unique_ptr<TopoDS_Shape> face_revolve(const TopoDS_Face& face,
    double ox, double oy, double oz,
    double dx, double dy, double dz,
    double angle);
std::unique_ptr<TopoDS_Shape> face_helix(const TopoDS_Face& face,
    double ox, double oy, double oz,
    double dx, double dy, double dz,
    double pitch, double turns, bool align_to_spine);

// ==================== Edge Methods ====================

ApproxPoints edge_approximation_segments(
    const TopoDS_Edge& edge, double tolerance);
ApproxPoints edge_approximation_segments_ex(
    const TopoDS_Edge& edge, double angular, double chord);

// ==================== Face Methods ====================

uint64_t face_tshape_id(const TopoDS_Face& face);

// ==================== SVG / HLR Projection ====================

SvgEdgeData project_shape_hlr(
    const TopoDS_Shape& shape,
    double dx, double dy, double dz,
    double tolerance);

} // namespace cadrum

#ifdef CADRUM_COLOR

namespace cadrum {

// ==================== Colored STEP I/O ====================

/// Result of read_step_color_stream.
/// shape  — the geometry compound.
/// ids    — TShape* addresses of colored faces (parallel to r/g/b).
/// r/g/b  — face color components in OCC native space (0.0–1.0).
class ColoredStepData {
public:
    TopoDS_Shape shape;
    std::vector<uint64_t> ids;
    std::vector<float>    r, g, b;
};

std::unique_ptr<ColoredStepData> read_step_color_stream(RustReader& reader);
std::unique_ptr<TopoDS_Shape>    colored_step_shape(const ColoredStepData& d);
rust::Vec<uint64_t>              colored_step_ids(const ColoredStepData& d);
rust::Vec<float>                 colored_step_colors_r(const ColoredStepData& d);
rust::Vec<float>                 colored_step_colors_g(const ColoredStepData& d);
rust::Vec<float>                 colored_step_colors_b(const ColoredStepData& d);

bool write_step_color_stream(
    const TopoDS_Shape&         shape,
    rust::Slice<const uint64_t> ids,
    rust::Slice<const float>    cr,
    rust::Slice<const float>    cg,
    rust::Slice<const float>    cb,
    RustWriter&                 writer);

// ==================== Clean with face-origin mapping ====================

/// Result of clean_shape_full: carries face-origin mapping for color remapping.
/// mapping is a flat array of [new_tshape_id, old_tshape_id, ...] pairs.
class CleanShape {
public:
    TopoDS_Shape shape;
    std::vector<uint64_t> mapping; // pairs: [new_id, old_id, ...]
};

std::unique_ptr<CleanShape> clean_shape_full(const TopoDS_Shape& shape);
std::unique_ptr<TopoDS_Shape> clean_shape_get(const CleanShape& r);
rust::Vec<uint64_t> clean_shape_mapping(const CleanShape& r);

} // namespace cadrum

#endif // CADRUM_COLOR

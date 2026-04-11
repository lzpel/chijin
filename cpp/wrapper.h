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

// Plain STEP I/O — only built without CADRUM_COLOR; with color, STEP goes
// through XCAF (`read_step_color_stream` etc.) instead.
#ifndef CADRUM_COLOR
std::unique_ptr<TopoDS_Shape> read_step_stream(RustReader& reader);
bool write_step_stream(const TopoDS_Shape& shape, RustWriter& writer);
#endif
std::unique_ptr<TopoDS_Shape> read_brep_bin_stream(RustReader& reader);
bool write_brep_bin_stream(const TopoDS_Shape& shape, RustWriter& writer);
std::unique_ptr<TopoDS_Shape> read_brep_text_stream(RustReader& reader);
bool write_brep_text_stream(const TopoDS_Shape& shape, RustWriter& writer);

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

std::unique_ptr<TopoDS_Shape> make_sphere(
    double cx, double cy, double cz,
    double radius);

std::unique_ptr<TopoDS_Shape> make_cone(
    double px, double py, double pz,
    double dx, double dy, double dz,
    double r1, double r2, double height);

std::unique_ptr<TopoDS_Shape> make_torus(
    double px, double py, double pz,
    double dx, double dy, double dz,
    double r1, double r2);

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

// Unified boolean operation: 0=Fuse(union), 1=Cut(a−b), 2=Common(intersect).
std::unique_ptr<BooleanShape> boolean_op(
    const TopoDS_Shape& a, const TopoDS_Shape& b, uint32_t op_kind);

std::unique_ptr<TopoDS_Shape> boolean_shape_shape(const BooleanShape& r);
rust::Vec<uint64_t> boolean_shape_from_a(const BooleanShape& r);
rust::Vec<uint64_t> boolean_shape_from_b(const BooleanShape& r);

// ==================== Shape Methods ====================

// Plain clean — only built without CADRUM_COLOR; with color, clean goes
// through `clean_shape_full` to remap face IDs onto the colormap.
#ifndef CADRUM_COLOR
std::unique_ptr<TopoDS_Shape> clean_shape(const TopoDS_Shape& shape);
#endif
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
void shape_bounding_box(const TopoDS_Shape& shape,
    double& xmin, double& ymin, double& zmin,
    double& xmax, double& ymax, double& zmax);

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

// ==================== Edge Methods ====================

// Approximate an edge as a polyline. The `_ex` variant is the implementation
// body and takes independent angular/chord deflection bounds. The plain
// variant is a convenience wrapper that maps a single `tolerance` value to
// both deflection bounds.
ApproxPoints edge_approximation_segments(
    const TopoDS_Edge& edge, double tolerance);
ApproxPoints edge_approximation_segments_ex(
    const TopoDS_Edge& edge, double angular, double chord);

// Construct a single helical edge on a cylindrical surface centered at the
// world origin. `axis` is the cylinder axis direction; `x_ref` is the
// reference direction that anchors the local +X axis of the cylindrical
// frame. The helix starts at `radius * normalize(x_ref - project_on(axis))`
// (i.e. at the +X side of the local frame, z=0) and rises by `height` over
// `height/pitch` turns. `x_ref` must not be parallel to `axis`.
std::unique_ptr<TopoDS_Edge> make_helix_edge(
    double ax, double ay, double az,
    double xrx, double xry, double xrz,
    double radius, double pitch, double height);

// Build a closed polygon from `coords` (flat xyz triples, ≥3 points) and
// return its constituent edges in order. The closing edge from the last
// point back to the first is included.
std::unique_ptr<std::vector<TopoDS_Edge>> make_polygon_edges(
    rust::Slice<const double> coords);

// Construct a closed circular edge of `radius` centered at the world origin,
// lying in the plane normal to `axis`. The local +X axis of the circle's
// frame (which determines the parametric start point) is chosen by OCCT
// from an arbitrary orthogonal direction to `axis`.
std::unique_ptr<TopoDS_Edge> make_circle_edge(
    double ax, double ay, double az, double radius);

// Construct a straight line segment edge from point a to point b.
std::unique_ptr<TopoDS_Edge> make_line_edge(
    double ax, double ay, double az,
    double bx, double by, double bz);

// Construct a circular arc edge through three points (start, mid, end).
// `mid` must not be collinear with `start` and `end`. On degenerate input
// OCCT returns nullptr.
std::unique_ptr<TopoDS_Edge> make_arc_edge(
    double sx, double sy, double sz,
    double mx, double my, double mz,
    double ex, double ey, double ez);

// Cubic B-spline edge interpolating data points.
//
// `coords` is a flat array of xyz triples (length must be a multiple of 3
// and ≥ 6). Each (x, y, z) is one interpolation target — the resulting
// curve passes through every input point exactly. `end_kind` selects the
// end-condition variant of `BSplineEnd`:
//   0 = Periodic (C² periodic; tangent args ignored)
//   1 = NotAKnot (open, OCCT default; tangent args ignored)
//   2 = Clamped  (open, explicit start/end tangents in (sx,sy,sz)/(ex,ey,ez))
// Returns nullptr on any failure.
std::unique_ptr<TopoDS_Edge> make_bspline_edge(
    rust::Slice<const double> coords,
    uint32_t end_kind,
    double sx, double sy, double sz,
    double ex, double ey, double ez);

// Edge query helpers.
void edge_start_point(const TopoDS_Edge& edge, double& x, double& y, double& z);
void edge_start_tangent(const TopoDS_Edge& edge, double& x, double& y, double& z);
bool edge_is_closed(const TopoDS_Edge& edge);

// Edge clone (deep copy of underlying TShape).
std::unique_ptr<TopoDS_Edge> deep_copy_edge(const TopoDS_Edge& edge);

// Edge spatial transforms. Mirror the shape-level helpers but operate on
// TopoDS_Edge directly so the Rust wrapper can stay edge-typed.
std::unique_ptr<TopoDS_Edge> translate_edge(
    const TopoDS_Edge& edge, double tx, double ty, double tz);
std::unique_ptr<TopoDS_Edge> rotate_edge(
    const TopoDS_Edge& edge,
    double ox, double oy, double oz,
    double dx, double dy, double dz,
    double angle);
std::unique_ptr<TopoDS_Edge> scale_edge(
    const TopoDS_Edge& edge,
    double cx, double cy, double cz,
    double factor);
std::unique_ptr<TopoDS_Edge> mirror_edge(
    const TopoDS_Edge& edge,
    double ox, double oy, double oz,
    double nx, double ny, double nz);

// Extrude a closed profile wire into a solid using BRepPrimAPI_MakePrism.
// Internally builds Wire → Face → Prism.
std::unique_ptr<TopoDS_Shape> make_extrude(
    const std::vector<TopoDS_Edge>& profile_edges,
    double dx, double dy, double dz);

// Sweep a closed profile wire (built from `profile_edges`) along a spine
// wire (built from `spine_edges`) using BRepOffsetAPI_MakePipeShell. The
// profile is wrapped in a face before sweeping so the result is a Solid.
// Unified MakePipeShell wrapper.  Supports single-profile sweep and
// multi-profile morphing.  Profile sections in `all_edges` are separated
// by null-edge sentinels (TopoDS_Edge().IsNull() == true).
//
// `orient` selects how the profile is oriented along the spine:
//   0 = Fixed   — fix the trihedron to the spine-start frame (no rotation)
//   1 = Torsion — raw Frenet trihedron (helices, springs)
//   2 = Up      — keep `(ux, uy, uz)` as the constant binormal direction
//   3 = Auxiliary — use `aux_spine_edges` as auxiliary spine for twist control
// Any other value falls back to Torsion.
std::unique_ptr<TopoDS_Shape> make_pipe_shell(
    const std::vector<TopoDS_Edge>& all_edges,
    const std::vector<TopoDS_Edge>& spine_edges,
    uint32_t orient,
    double ux, double uy, double uz,
    const std::vector<TopoDS_Edge>& aux_spine_edges);

// Helpers for the Rust side to construct a std::vector<TopoDS_Edge>.
std::unique_ptr<std::vector<TopoDS_Edge>> edge_vec_new();
void edge_vec_push(std::vector<TopoDS_Edge>& v, const TopoDS_Edge& e);
void edge_vec_push_null(std::vector<TopoDS_Edge>& v);

// Loft (skin) a smooth solid through N cross-section wires.
// Sections in `all_edges` are separated by null-edge sentinels.
std::unique_ptr<TopoDS_Shape> make_loft(
    const std::vector<TopoDS_Edge>& all_edges);

// Gordon surface solid from a network of profile and guide curves.
// Both `profile_edges` and `guide_edges` use null-edge sentinels to separate
// individual curves (each curve may consist of multiple edges forming a wire).
// Requires ≥ 2 profiles and ≥ 2 guides, all mutually intersecting.
std::unique_ptr<TopoDS_Shape> make_gordon(
    const std::vector<TopoDS_Edge>& profile_edges,
    const std::vector<TopoDS_Edge>& guide_edges);

// ==================== Face Methods ====================

// Both helpers return the underlying TopoDS_TShape* address as a u64 — used
// to track face/solid identity across boolean ops, color maps, and BREP I/O.
uint64_t face_tshape_id(const TopoDS_Face& face);
uint64_t shape_tshape_id(const TopoDS_Shape& shape);

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

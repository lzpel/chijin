#pragma once

#include "rust/cxx.h"

#include <TopoDS_Shape.hxx>
#include <TopoDS_Face.hxx>
#include <TopoDS_Edge.hxx>
#include <TopoDS_Solid.hxx>
#include <TopoDS_Compound.hxx>
#include <TopExp_Explorer.hxx>
#include <TopAbs_ShapeEnum.hxx>
#include <TopoDS.hxx>

#include <BRepBuilderAPI_Copy.hxx>
#include <BRepBuilderAPI_MakeFace.hxx>
#include <BRepPrimAPI_MakeBox.hxx>
#include <BRepPrimAPI_MakeCylinder.hxx>
#include <BRepPrimAPI_MakeHalfSpace.hxx>
#include <BRepPrimAPI_MakePrism.hxx>

#include <BRepAlgoAPI_Fuse.hxx>
#include <BRepAlgoAPI_Cut.hxx>
#include <BRepAlgoAPI_Common.hxx>

#include <ShapeUpgrade_UnifySameDomain.hxx>

#include <BRepMesh_IncrementalMesh.hxx>
#include <BRep_Tool.hxx>
#include <Poly_Triangulation.hxx>
#include <BRepGProp.hxx>
#include <BRepGProp_Face.hxx>
#include <GProp_GProps.hxx>
#include <GeomAPI_ProjectPointOnSurf.hxx>

#include <BRepAdaptor_Curve.hxx>
#include <GCPnts_TangentialDeflection.hxx>

#include <BRep_Builder.hxx>
#include <gp_Pln.hxx>
#include <gp_Ax2.hxx>
#include <gp_Trsf.hxx>
#include <TopLoc_Location.hxx>

#include <BinTools.hxx>
#include <BRepTools.hxx>
#include <STEPControl_Reader.hxx>
#include <Message_ProgressRange.hxx>

#include <BRepBuilderAPI_Transform.hxx>

#include <streambuf>
#include <istream>
#include <ostream>
#include <memory>

// Forward-declare the Rust opaque types
struct RustReader;
struct RustWriter;

// Forward-declare the Rust FFI callbacks
size_t rust_reader_read(RustReader& reader, rust::Slice<uint8_t> buf);
size_t rust_writer_write(RustWriter& writer, rust::Slice<const uint8_t> buf);
bool rust_writer_flush(RustWriter& writer);

namespace chijin {

// ==================== Streambuf bridges ====================

// std::streambuf subclass that reads from a Rust `dyn Read` via FFI callback
class RustReadStreambuf : public std::streambuf {
public:
    explicit RustReadStreambuf(RustReader& reader) : reader_(reader) {}

protected:
    int_type underflow() override {
        rust::Slice<uint8_t> slice(
            reinterpret_cast<uint8_t*>(buf_), sizeof(buf_));
        size_t n = rust_reader_read(reader_, slice);
        if (n == 0) return traits_type::eof();
        setg(buf_, buf_, buf_ + n);
        return traits_type::to_int_type(*gptr());
    }

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
    int_type overflow(int_type ch) override {
        if (ch != traits_type::eof()) {
            buf_[pos_++] = static_cast<char>(ch);
            if (pos_ >= sizeof(buf_)) {
                if (!flush_buf()) return traits_type::eof();
            }
        }
        return ch;
    }

    std::streamsize xsputn(const char* s, std::streamsize count) override {
        std::streamsize written = 0;
        while (written < count) {
            std::streamsize space = sizeof(buf_) - pos_;
            std::streamsize chunk = std::min(count - written, space);
            std::memcpy(buf_ + pos_, s + written, chunk);
            pos_ += static_cast<size_t>(chunk);
            written += chunk;
            if (pos_ >= sizeof(buf_)) {
                if (!flush_buf()) return written;
            }
        }
        return written;
    }

    int sync() override {
        return flush_buf() ? 0 : -1;
    }

private:
    bool flush_buf() {
        if (pos_ == 0) return true;
        rust::Slice<const uint8_t> slice(
            reinterpret_cast<const uint8_t*>(buf_), pos_);
        size_t n = rust_writer_write(writer_, slice);
        if (n < pos_) return false;
        pos_ = 0;
        return true;
    }

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

// ==================== Boolean Operations ====================

std::unique_ptr<TopoDS_Shape> boolean_fuse(
    const TopoDS_Shape& a, const TopoDS_Shape& b);
std::unique_ptr<TopoDS_Shape> boolean_cut(
    const TopoDS_Shape& a, const TopoDS_Shape& b);
std::unique_ptr<TopoDS_Shape> boolean_common(
    const TopoDS_Shape& a, const TopoDS_Shape& b);

// ==================== Shape Methods ====================

std::unique_ptr<TopoDS_Shape> clean_shape(const TopoDS_Shape& shape);
std::unique_ptr<TopoDS_Shape> translate_shape(
    const TopoDS_Shape& shape, double tx, double ty, double tz);
bool shape_is_null(const TopoDS_Shape& shape);

// ==================== Meshing ====================

struct MeshData {
    rust::Vec<double> vertices;
    rust::Vec<double> uvs;
    rust::Vec<double> normals;
    rust::Vec<uint32_t> indices;
    bool success;
};

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
std::unique_ptr<TopoDS_Shape> face_extrude(const TopoDS_Face& face,
    double dx, double dy, double dz);
std::unique_ptr<TopoDS_Shape> face_to_shape(const TopoDS_Face& face);

// ==================== Edge Methods ====================

struct ApproxPoints {
    rust::Vec<double> coords;
    uint32_t count;
};

ApproxPoints edge_approximation_segments(
    const TopoDS_Edge& edge, double tolerance);

} // namespace chijin

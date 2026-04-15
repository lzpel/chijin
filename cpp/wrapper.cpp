#include "cadrum/src/occt/ffi.rs.h"

// ==================== OCCT headers (impl only — not exposed via wrapper.h) ====================
//
// Grouped by responsibility. Anything used in wrapper.h is included there;
// here we only pull in what the implementations need.

// --- Standard / exceptions ---
#include <Standard_Failure.hxx>

// --- Topology types & navigation ---
#include <TopoDS.hxx>
#include <TopoDS_Compound.hxx>
#include <TopAbs_ShapeEnum.hxx>
#include <TopExp.hxx>
#include <TopLoc_Location.hxx>
#include <NCollection_IndexedMap.hxx>
#include <NCollection_List.hxx>
#include <TopTools_ShapeMapHasher.hxx>

// --- Geometry primitives (gp / Geom / 2d) ---
#include <gp_Ax1.hxx>
#include <gp_Ax2.hxx>
#include <gp_Circ.hxx>
#include <gp_Lin.hxx>
#include <gp_Pln.hxx>
#include <gp_Trsf.hxx>
#include <Geom_CylindricalSurface.hxx>
#include <Geom2d_Line.hxx>
#include <GC_MakeArcOfCircle.hxx>

// --- BRep builders (faces / wires / edges / solid primitives) ---
#include <BRep_Builder.hxx>
#include <BRep_Tool.hxx>
#include <BRepLib.hxx>
#include <BRepBuilderAPI_Copy.hxx>
#include <BRepBuilderAPI_MakeFace.hxx>
#include <BRepBuilderAPI_MakePolygon.hxx>
#include <BRepBuilderAPI_MakeEdge.hxx>
#include <BRepBuilderAPI_MakeWire.hxx>
#include <BRepBuilderAPI_MakeSolid.hxx>
#include <BRepBuilderAPI_Sewing.hxx>
#include <BRepBuilderAPI_Transform.hxx>
#include <BRepClass3d_SolidClassifier.hxx>
#include <BRepPrimAPI_MakeBox.hxx>
#include <BRepPrimAPI_MakeCone.hxx>
#include <BRepPrimAPI_MakeCylinder.hxx>
#include <BRepPrimAPI_MakeHalfSpace.hxx>
#include <BRepPrimAPI_MakeSphere.hxx>
#include <BRepPrimAPI_MakePrism.hxx>
#include <BRepPrimAPI_MakeTorus.hxx>

// --- Boolean operations & shape cleanup ---
#include <BRepAlgoAPI_BooleanOperation.hxx>
#include <BRepAlgoAPI_Fuse.hxx>
#include <BRepAlgoAPI_Cut.hxx>
#include <BRepAlgoAPI_Common.hxx>
#include <ShapeUpgrade_UnifySameDomain.hxx>
#include <BRepTools_History.hxx>

// --- Sweep / pipe / loft ---
#include <BRepOffsetAPI_MakePipeShell.hxx>
#include <BRepOffsetAPI_ThruSections.hxx>

// --- Mesh, classification, mass / surface properties ---
#include <BRepMesh_IncrementalMesh.hxx>
#include <Poly_Triangulation.hxx>
#include <BRepClass3d_SolidClassifier.hxx>
#include <BRepBndLib.hxx>
#include <Bnd_Box.hxx>
#include <BRepGProp.hxx>
#include <GProp_GProps.hxx>

// --- Curve adaptation / approximation ---
#include <BRepAdaptor_Curve.hxx>
#include <GCPnts_TangentialDeflection.hxx>
#include <GeomAPI_Interpolate.hxx>
#include <GeomAPI_PointsToBSplineSurface.hxx>
#include <Geom_BSplineCurve.hxx>
#include <Geom_BSplineSurface.hxx>
#include <NCollection_Array2.hxx>
#include <NCollection_HArray1.hxx>
#include <Precision.hxx>

// --- I/O (BREP / STEP / progress) ---
#include <BRepTools.hxx>
#include <BinTools.hxx>
#include <STEPControl_Reader.hxx>
#include <STEPControl_Writer.hxx>
#include <Message_ProgressRange.hxx>

// --- C++ standard library ---
#include <istream>
#include <ostream>
#include <sstream>
#include <cmath>
#include <cstring>
#include <cstdint>
#include <algorithm>
#include <unordered_map>
#include <unordered_set>
#include <array>

namespace cadrum {

// ==================== RustReadStreambuf ====================

std::streambuf::int_type RustReadStreambuf::underflow() {
    rust::Slice<uint8_t> slice(
        reinterpret_cast<uint8_t*>(buf_), sizeof(buf_));
    size_t n = rust_reader_read(reader_, slice);
    if (n == 0) return traits_type::eof();
    setg(buf_, buf_, buf_ + n);
    return traits_type::to_int_type(*gptr());
}

std::streambuf::pos_type RustReadStreambuf::seekpos(pos_type, std::ios_base::openmode) {
    return pos_type(off_type(-1));
}

// ==================== RustWriteStreambuf ====================

std::streambuf::int_type RustWriteStreambuf::overflow(int_type ch) {
    if (ch != traits_type::eof()) {
        buf_[pos_++] = static_cast<char>(ch);
        if (pos_ >= sizeof(buf_)) {
            if (!flush_buf()) return traits_type::eof();
        }
    }
    return ch;
}

std::streamsize RustWriteStreambuf::xsputn(const char* s, std::streamsize count) {
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

int RustWriteStreambuf::sync() {
    return flush_buf() ? 0 : -1;
}

bool RustWriteStreambuf::flush_buf() {
    if (pos_ == 0) return true;
    rust::Slice<const uint8_t> slice(
        reinterpret_cast<const uint8_t*>(buf_), pos_);
    size_t n = rust_writer_write(writer_, slice);
    if (n < pos_) return false;
    pos_ = 0;
    return true;
}

std::streambuf::pos_type RustWriteStreambuf::seekpos(pos_type, std::ios_base::openmode) {
    return pos_type(off_type(-1));
}

// ==================== Shape I/O (streambuf callback) ====================

#ifndef CADRUM_COLOR
// Plain STEP I/O — used only when CADRUM_COLOR is not defined.
// With color, STEP routes through XCAF (`read_step_color_stream` /
// `write_step_color_stream`) instead.

std::unique_ptr<TopoDS_Shape> read_step_stream(RustReader& reader) {
    RustReadStreambuf sbuf(reader);
    std::istream is(&sbuf);

    // OCCT 7.x bug workaround: STEPControl_Reader::~STEPControl_Reader()
    // crashes when the reader was constructed on the stack and destroyed
    // after a successful TransferRoots(). Allocating on the heap and never
    // freeing avoids the destructor path entirely. The leaked memory is
    // bounded (one reader per STEP read) and accepted as a known cost.
    auto* step_reader = new STEPControl_Reader();
    IFSelect_ReturnStatus status = step_reader->ReadStream("stream", is);

    if (status != IFSelect_RetDone) {
        return nullptr;
    }

    step_reader->TransferRoots(Message_ProgressRange());
    return std::make_unique<TopoDS_Shape>(step_reader->OneShape());
    // step_reader is intentionally leaked — see comment above.
}

bool write_step_stream(const TopoDS_Shape& shape, RustWriter& writer) {
    RustWriteStreambuf sbuf(writer);
    std::ostream os(&sbuf);
    STEPControl_Writer step_writer;
    if (step_writer.Transfer(shape, STEPControl_AsIs) != IFSelect_RetDone) {
        return false;
    }
    return step_writer.WriteStream(os) == IFSelect_RetDone;
}
#endif // !CADRUM_COLOR

std::unique_ptr<TopoDS_Shape> read_brep_bin_stream(RustReader& reader) {
    // BinTools::Read requires a seekable stream: the binary format stores
    // backward references (offsets) for shared sub-shapes and seeks to them.
    // Our RustReadStreambuf is sequential only, so buffer everything in
    // memory first and use std::istringstream which is seekable.
    std::string data;
    char buf[8192];
    for (;;) {
        rust::Slice<uint8_t> slice(reinterpret_cast<uint8_t*>(buf), sizeof(buf));
        size_t n = rust_reader_read(reader, slice);
        if (n == 0) break;
        data.append(buf, n);
    }

    std::istringstream iss(std::move(data));
    auto shape = std::make_unique<TopoDS_Shape>();
    try {
        BinTools::Read(*shape, iss);
    } catch (const Standard_Failure&) {
        return nullptr;
    }

    if (shape->IsNull()) {
        return nullptr;
    }
    return shape;
}

bool write_brep_bin_stream(const TopoDS_Shape& shape, RustWriter& writer) {
    RustWriteStreambuf sbuf(writer);
    std::ostream os(&sbuf);
    try {
        BinTools::Write(shape, os);
    } catch (const Standard_Failure&) {
        return false;
    }
    return os.good();
}

std::unique_ptr<TopoDS_Shape> read_brep_text_stream(RustReader& reader) {
    RustReadStreambuf sbuf(reader);
    std::istream is(&sbuf);

    auto shape = std::make_unique<TopoDS_Shape>();
    BRep_Builder builder;
    try {
        BRepTools::Read(*shape, is, builder);
    } catch (const Standard_Failure&) {
        return nullptr;
    }

    if (shape->IsNull()) {
        return nullptr;
    }
    return shape;
}

bool write_brep_text_stream(const TopoDS_Shape& shape, RustWriter& writer) {
    RustWriteStreambuf sbuf(writer);
    std::ostream os(&sbuf);
    try {
        BRepTools::Write(shape, os);
    } catch (const Standard_Failure&) {
        return false;
    }
    return os.good();
}

// ==================== Shape Constructors ====================

std::unique_ptr<TopoDS_Shape> make_half_space(
    double ox, double oy, double oz,
    double nx, double ny, double nz)
{
    gp_Pnt origin(ox, oy, oz);
    gp_Dir normal(nx, ny, nz);
    gp_Pln plane(origin, normal);

    BRepBuilderAPI_MakeFace face_maker(plane);
    TopoDS_Face face = face_maker.Face();

    // Reference point is on the SAME side as the normal.
    // BRepPrimAPI_MakeHalfSpace fills the ref_point side,
    // so the solid occupies the half-space where the normal points.
    double len = std::sqrt(nx*nx + ny*ny + nz*nz);
    gp_Pnt ref_point(ox + nx/len, oy + ny/len, oz + nz/len);

    BRepPrimAPI_MakeHalfSpace maker(face, ref_point);
    return std::make_unique<TopoDS_Shape>(maker.Solid());
}

std::unique_ptr<TopoDS_Shape> make_box(
    double x1, double y1, double z1,
    double x2, double y2, double z2)
{
    double minx = std::min(x1, x2);
    double miny = std::min(y1, y2);
    double minz = std::min(z1, z2);
    double maxx = std::max(x1, x2);
    double maxy = std::max(y1, y2);
    double maxz = std::max(z1, z2);

    gp_Pnt p_min(minx, miny, minz);
    double dx = maxx - minx;
    double dy = maxy - miny;
    double dz = maxz - minz;

    BRepPrimAPI_MakeBox maker(p_min, dx, dy, dz);
    return std::make_unique<TopoDS_Shape>(maker.Shape());
}

std::unique_ptr<TopoDS_Shape> make_cylinder(
    double px, double py, double pz,
    double dx, double dy, double dz,
    double radius, double height)
{
    gp_Pnt center(px, py, pz);
    gp_Dir direction(dx, dy, dz);
    gp_Ax2 axis(center, direction);

    BRepPrimAPI_MakeCylinder maker(axis, radius, height);
    return std::make_unique<TopoDS_Shape>(maker.Shape());
}

std::unique_ptr<TopoDS_Shape> make_sphere(
    double cx, double cy, double cz,
    double radius)
{
    gp_Pnt center(cx, cy, cz);
    BRepPrimAPI_MakeSphere maker(center, radius);
    return std::make_unique<TopoDS_Shape>(maker.Shape());
}

std::unique_ptr<TopoDS_Shape> make_cone(
    double px, double py, double pz,
    double dx, double dy, double dz,
    double r1, double r2, double height)
{
    gp_Pnt center(px, py, pz);
    gp_Dir direction(dx, dy, dz);
    gp_Ax2 axis(center, direction);
    BRepPrimAPI_MakeCone maker(axis, r1, r2, height);
    return std::make_unique<TopoDS_Shape>(maker.Shape());
}

std::unique_ptr<TopoDS_Shape> make_torus(
    double px, double py, double pz,
    double dx, double dy, double dz,
    double r1, double r2)
{
    gp_Pnt center(px, py, pz);
    gp_Dir direction(dx, dy, dz);
    gp_Ax2 axis(center, direction);
    BRepPrimAPI_MakeTorus maker(axis, r1, r2);
    return std::make_unique<TopoDS_Shape>(maker.Shape());
}

std::unique_ptr<TopoDS_Shape> make_empty() {
    TopoDS_Compound compound;
    BRep_Builder builder;
    builder.MakeCompound(compound);
    return std::make_unique<TopoDS_Shape>(compound);
}

std::unique_ptr<TopoDS_Shape> deep_copy(const TopoDS_Shape& shape) {
    BRepBuilderAPI_Copy copier(shape, true, false);
    return std::make_unique<TopoDS_Shape>(copier.Shape());
}

std::unique_ptr<TopoDS_Shape> shallow_copy(const TopoDS_Shape& shape) {
    return std::make_unique<TopoDS_Shape>(shape);
}

// ==================== Compound Decompose/Compose ====================

std::unique_ptr<std::vector<TopoDS_Shape>> decompose_into_solids(const TopoDS_Shape& shape) {
    auto result = std::make_unique<std::vector<TopoDS_Shape>>();
    for (TopExp_Explorer ex(shape, TopAbs_SOLID); ex.More(); ex.Next()) {
        result->push_back(ex.Current());  // shallow handle copy
    }
    return result;
}

void compound_add(TopoDS_Shape& compound, const TopoDS_Shape& child) {
    BRep_Builder builder;
    builder.Add(compound, child);
}

// ==================== Boolean Operations ====================
// Bug 1 fix: All boolean results are deep-copied via BRepBuilderAPI_Copy
// so the result shares no Handle<Geom_XXX> with the input shapes.
// This prevents STATUS_HEAP_CORRUPTION when shapes are dropped in any order.
//
// Cross-section face collection: Modified() is called BEFORE BRepBuilderAPI_Copy
// because the copy severs the history table. Each collected face is then
// individually deep-copied so it is independent of the operator object.
//
// Why Modified() and not Generated():
//   The cross-section face is the tool's boundary face trimmed (bounded) to fit
//   inside the shape operand.  OCCT records this as Modified(tool_face) because
//   the face still represents the same plane — it just has smaller bounds.
//   Generated(tool_face) returns empty because no wholly NEW face was created.
//
// from_a / from_b (修正案2):
//   For each face in src, collect_relay_mapping builds a map from the
//   pre-copy TShape* of the result face to the TShape* of the original src face.
//   After BRepBuilderAPI_Copy, copier.ModifiedShape() maps pre→post copy.
//   The combined mapping (src → pre → post) is stored as flat [post_id, src_id] pairs.

// Helper: build relay map  pre_copy_result_tshape* → src_tshape*
// Called before BRepBuilderAPI_Copy, while op history is alive.
static void collect_relay_mapping(
    BRepAlgoAPI_BooleanOperation& op,
    const TopoDS_Shape& src,
    std::unordered_map<uint64_t, uint64_t>& relay)
{
    for (TopExp_Explorer ex(src, TopAbs_FACE); ex.More(); ex.Next()) {
        const TopoDS_Shape& sf = ex.Current();
        uint64_t src_id = reinterpret_cast<uint64_t>(sf.TShape().get());
        if (op.IsDeleted(sf)) continue;
        const NCollection_List<TopoDS_Shape>& mods = op.Modified(sf);
        if (mods.IsEmpty()) {
            // Face is unchanged: its TShape* appears as-is in op.Shape().
            relay[src_id] = src_id;
        } else {
            for (NCollection_List<TopoDS_Shape>::Iterator it(mods); it.More(); it.Next()) {
                uint64_t pre_id = reinterpret_cast<uint64_t>(it.Value().TShape().get());
                relay[pre_id] = src_id;
            }
        }
    }
}

// Helper: after BRepBuilderAPI_Copy, match pre/post faces by their index in
// NCollection_IndexedMap<TopoDS_Shape, TopTools_ShapeMapHasher> (BRepBuilderAPI_Copy preserves traversal order).
// Emit [post_id, src_id] pairs into `out` for every face tracked in `relay`.
static void emit_from_pairs(
    const TopoDS_Shape& pre_shape,
    const TopoDS_Shape& post_shape,
    const std::unordered_map<uint64_t, uint64_t>& relay,
    std::vector<uint64_t>& out)
{
    NCollection_IndexedMap<TopoDS_Shape, TopTools_ShapeMapHasher> pre_map, post_map;
    TopExp::MapShapes(pre_shape, TopAbs_FACE, pre_map);
    TopExp::MapShapes(post_shape, TopAbs_FACE, post_map);
    // pre_map and post_map have the same size because the copy preserves topology.
    for (int i = 1; i <= pre_map.Size(); ++i) {
        uint64_t pre_id = reinterpret_cast<uint64_t>(pre_map(i).TShape().get());
        auto it = relay.find(pre_id);
        if (it == relay.end()) continue;
        uint64_t post_id = reinterpret_cast<uint64_t>(post_map(i).TShape().get());
        out.push_back(post_id);
        out.push_back(it->second);
    }
}

// Unified boolean operation. `op_kind` selects the OCCT algorithm:
//   0 = Fuse (union)
//   1 = Cut  (subtract: a − b)
//   2 = Common (intersect)
// Any other value returns nullptr.
//
// All three OCCT operations derive from BRepAlgoAPI_BooleanOperation, so the
// post-build relay/copy logic is identical. Branching only at construction
// avoids triplicating the bookkeeping.
std::unique_ptr<BooleanShape> boolean_op(
    const TopoDS_Shape& a, const TopoDS_Shape& b, uint32_t op_kind)
{
    try {
        std::unique_ptr<BRepAlgoAPI_BooleanOperation> op;
        switch (op_kind) {
            case 0: op = std::make_unique<BRepAlgoAPI_Fuse>(a, b); break;
            case 1: op = std::make_unique<BRepAlgoAPI_Cut>(a, b); break;
            case 2: op = std::make_unique<BRepAlgoAPI_Common>(a, b); break;
            default: return nullptr;
        }
        op->Build();
        if (!op->IsDone()) return nullptr;

        std::unordered_map<uint64_t, uint64_t> relay_a, relay_b;
        collect_relay_mapping(*op, a, relay_a);
        collect_relay_mapping(*op, b, relay_b);

        BRepBuilderAPI_Copy copier(op->Shape(), true, false);
        auto r = std::make_unique<BooleanShape>();
        r->shape = copier.Shape();
        emit_from_pairs(op->Shape(), copier.Shape(), relay_a, r->from_a);
        emit_from_pairs(op->Shape(), copier.Shape(), relay_b, r->from_b);
        return r;
    } catch (const Standard_Failure&) {
        return nullptr;
    }
}

std::unique_ptr<TopoDS_Shape> boolean_shape_shape(const BooleanShape& r) {
    return std::make_unique<TopoDS_Shape>(r.shape);
}

rust::Vec<uint64_t> boolean_shape_from_a(const BooleanShape& r) {
    rust::Vec<uint64_t> v;
    for (uint64_t x : r.from_a) v.push_back(x);
    return v;
}

rust::Vec<uint64_t> boolean_shape_from_b(const BooleanShape& r) {
    rust::Vec<uint64_t> v;
    for (uint64_t x : r.from_b) v.push_back(x);
    return v;
}

// ==================== Shape Methods ====================

#ifndef CADRUM_COLOR
// Plain clean — used only when CADRUM_COLOR is not defined.
// With color, clean goes through `clean_shape_full` which also returns a
// face-id remapping table so the colormap can follow merged faces.
std::unique_ptr<TopoDS_Shape> clean_shape(const TopoDS_Shape& shape) {
    try {
        ShapeUpgrade_UnifySameDomain unifier(shape, true, true, true);
        unifier.AllowInternalEdges(false);
        unifier.Build();
        return std::make_unique<TopoDS_Shape>(unifier.Shape());
    } catch (const Standard_Failure&) {
        return nullptr;
    }
}
#endif // !CADRUM_COLOR

std::unique_ptr<TopoDS_Shape> translate_shape(
    const TopoDS_Shape& shape,
    double tx, double ty, double tz)
{
    gp_Trsf trsf;
    trsf.SetTranslation(gp_Vec(tx, ty, tz));
    return std::make_unique<TopoDS_Shape>(shape.Moved(TopLoc_Location(trsf)));
}

std::unique_ptr<TopoDS_Shape> rotate_shape(
    const TopoDS_Shape& shape,
    double ox, double oy, double oz,
    double dx, double dy, double dz,
    double angle)
{
    try {
        gp_Trsf trsf;
        trsf.SetRotation(gp_Ax1(gp_Pnt(ox, oy, oz), gp_Dir(dx, dy, dz)), angle);
        return std::make_unique<TopoDS_Shape>(shape.Moved(TopLoc_Location(trsf)));
    } catch (const Standard_Failure&) {
        return nullptr;
    }
}

std::unique_ptr<TopoDS_Shape> scale_shape(
    const TopoDS_Shape& shape,
    double cx, double cy, double cz,
    double factor)
{
    try {
        gp_Trsf trsf;
        trsf.SetScale(gp_Pnt(cx, cy, cz), factor);
        BRepBuilderAPI_Transform transform(shape, trsf, true);
        return std::make_unique<TopoDS_Shape>(transform.Shape());
    } catch (const Standard_Failure&) {
        return nullptr;
    }
}

std::unique_ptr<TopoDS_Shape> mirror_shape(
    const TopoDS_Shape& shape,
    double ox, double oy, double oz,
    double nx, double ny, double nz)
{
    try {
        gp_Trsf trsf;
        trsf.SetMirror(gp_Ax2(gp_Pnt(ox, oy, oz), gp_Dir(nx, ny, nz)));
        BRepBuilderAPI_Transform transform(shape, trsf, true);
        return std::make_unique<TopoDS_Shape>(transform.Shape());
    } catch (const Standard_Failure&) {
        return nullptr;
    }
}

bool shape_is_null(const TopoDS_Shape& shape) {
    return shape.IsNull();
}

bool shape_is_solid(const TopoDS_Shape& shape) {
    return !shape.IsNull() && shape.ShapeType() == TopAbs_SOLID;
}

uint32_t shape_shell_count(const TopoDS_Shape& shape) {
    uint32_t count = 0;
    for (TopExp_Explorer ex(shape, TopAbs_SHELL); ex.More(); ex.Next()) {
        ++count;
    }
    return count;
}

double shape_volume(const TopoDS_Shape& shape) {
    GProp_GProps props;
    BRepGProp::VolumeProperties(shape, props);
    return props.Mass();
}

bool shape_contains_point(const TopoDS_Shape& shape, double x, double y, double z) {
    BRepClass3d_SolidClassifier classifier(shape, gp_Pnt(x, y, z), 1e-6);
    return classifier.State() == TopAbs_IN;
}

void shape_bounding_box(const TopoDS_Shape& shape,
    double& xmin, double& ymin, double& zmin,
    double& xmax, double& ymax, double& zmax)
{
    Bnd_Box box;
    BRepBndLib::Add(shape, box);
    box.Get(xmin, ymin, zmin, xmax, ymax, zmax);
}

// ==================== Meshing ====================

MeshData mesh_shape(const TopoDS_Shape& shape, double tolerance) {
    MeshData result;
    result.success = false;

    BRepMesh_IncrementalMesh mesher(shape, tolerance);
    if (!mesher.IsDone()) {
        return result;
    }

    uint32_t global_vertex_offset = 0;

    for (TopExp_Explorer explorer(shape, TopAbs_FACE); explorer.More(); explorer.Next()) {
        TopoDS_Face face = TopoDS::Face(explorer.Current());
        TopLoc_Location location;
        Handle(Poly_Triangulation) triangulation = BRep_Tool::Triangulation(face, location);

        if (triangulation.IsNull()) {
            continue;
        }

        int nb_nodes = triangulation->NbNodes();
        int nb_triangles = triangulation->NbTriangles();

        // Compute normals for this face
        // Bug 3 fix: Use Poly_Triangulation::ComputeNormals + correct loop bounds

        // Vertices
        for (int i = 1; i <= nb_nodes; i++) {
            gp_Pnt p = triangulation->Node(i);
            p.Transform(location.Transformation());
            result.vertices.push_back(p.X());
            result.vertices.push_back(p.Y());
            result.vertices.push_back(p.Z());
        }

        // UVs - normalize per face
        if (triangulation->HasUVNodes()) {
            double u_min = 1e30, u_max = -1e30, v_min = 1e30, v_max = -1e30;
            for (int i = 1; i <= nb_nodes; i++) {
                gp_Pnt2d uv = triangulation->UVNode(i);
                u_min = std::min(u_min, uv.X());
                u_max = std::max(u_max, uv.X());
                v_min = std::min(v_min, uv.Y());
                v_max = std::max(v_max, uv.Y());
            }
            double u_range = u_max - u_min;
            double v_range = v_max - v_min;
            if (u_range < 1e-10) u_range = 1.0;
            if (v_range < 1e-10) v_range = 1.0;

            for (int i = 1; i <= nb_nodes; i++) {
                gp_Pnt2d uv = triangulation->UVNode(i);
                result.uvs.push_back((uv.X() - u_min) / u_range);
                result.uvs.push_back((uv.Y() - v_min) / v_range);
            }
        } else {
            for (int i = 1; i <= nb_nodes; i++) {
                result.uvs.push_back(0.0);
                result.uvs.push_back(0.0);
            }
        }

        // Normals - Bug 3 fix: iterate exactly nb_nodes times (1..=nb_nodes)
        // Previous code used normal_array.Length() which was off-by-one.
        if (!triangulation->HasNormals()) {
            triangulation->ComputeNormals();
        }
        for (int i = 1; i <= nb_nodes; i++) {
            gp_Dir normal = triangulation->Normal(i);
            if (face.Orientation() == TopAbs_REVERSED) {
                result.normals.push_back(-normal.X());
                result.normals.push_back(-normal.Y());
                result.normals.push_back(-normal.Z());
            } else {
                result.normals.push_back(normal.X());
                result.normals.push_back(normal.Y());
                result.normals.push_back(normal.Z());
            }
        }

        // Indices
        bool reversed = (face.Orientation() == TopAbs_REVERSED);
        uint64_t face_id = reinterpret_cast<uint64_t>(face.TShape().get());
        for (int i = 1; i <= nb_triangles; i++) {
            const Poly_Triangle& tri = triangulation->Triangle(i);

            int n1, n2, n3;
            tri.Get(n1, n2, n3);

            // OCC indices are 1-based, convert to 0-based + global offset
            if (reversed) {
                result.indices.push_back(global_vertex_offset + n1 - 1);
                result.indices.push_back(global_vertex_offset + n3 - 1);
                result.indices.push_back(global_vertex_offset + n2 - 1);
            } else {
                result.indices.push_back(global_vertex_offset + n1 - 1);
                result.indices.push_back(global_vertex_offset + n2 - 1);
                result.indices.push_back(global_vertex_offset + n3 - 1);
            }
            result.face_tshape_ids.push_back(face_id);
        }

        global_vertex_offset += nb_nodes;
    }

    result.success = true;
    return result;
}

// ==================== Explorer / Iterators ====================

std::unique_ptr<TopExp_Explorer> explore_faces(const TopoDS_Shape& shape) {
    return std::make_unique<TopExp_Explorer>(shape, TopAbs_FACE);
}

std::unique_ptr<TopExp_Explorer> explore_edges(const TopoDS_Shape& shape) {
    // TopExp_Explorer visits shared edges once per adjacent face.
    // Build a flat compound of unique edges so each is visited exactly once.
    NCollection_IndexedMap<TopoDS_Shape, TopTools_ShapeMapHasher> edgeMap;
    TopExp::MapShapes(shape, TopAbs_EDGE, edgeMap);
    TopoDS_Compound compound;
    BRep_Builder builder;
    builder.MakeCompound(compound);
    for (int i = 1; i <= edgeMap.Extent(); i++) {
        builder.Add(compound, edgeMap(i));
    }
    return std::make_unique<TopExp_Explorer>(compound, TopAbs_EDGE);
}

bool explorer_more(const TopExp_Explorer& explorer) {
    return explorer.More();
}

void explorer_next(TopExp_Explorer& explorer) {
    explorer.Next();
}

std::unique_ptr<TopoDS_Face> explorer_current_face(const TopExp_Explorer& explorer) {
    return std::make_unique<TopoDS_Face>(TopoDS::Face(explorer.Current()));
}

std::unique_ptr<TopoDS_Edge> explorer_current_edge(const TopExp_Explorer& explorer) {
    return std::make_unique<TopoDS_Edge>(TopoDS::Edge(explorer.Current()));
}

// ==================== Face Methods ====================

uint64_t face_tshape_id(const TopoDS_Face& face) {
    return reinterpret_cast<uint64_t>(face.TShape().get());
}

uint64_t shape_tshape_id(const TopoDS_Shape& shape) {
    return reinterpret_cast<uint64_t>(shape.TShape().get());
}

// ==================== Edge Methods ====================

ApproxPoints edge_approximation_segments_ex(
    const TopoDS_Edge& edge, double angular, double chord)
{
    ApproxPoints result;
    result.count = 0;

    try {
        BRepAdaptor_Curve curve(edge);
        GCPnts_TangentialDeflection approx(curve, angular, chord);

        int nb_points = approx.NbPoints();
        result.count = static_cast<uint32_t>(nb_points);

        for (int i = 1; i <= nb_points; i++) {
            gp_Pnt p = approx.Value(i);
            result.coords.push_back(p.X());
            result.coords.push_back(p.Y());
            result.coords.push_back(p.Z());
        }
    } catch (const Standard_Failure&) {
        result.count = 0;
        result.coords.clear();
    }

    return result;
}

ApproxPoints edge_approximation_segments(
    const TopoDS_Edge& edge, double tolerance)
{
    // Bug 4 fix: tolerance is now a parameter instead of hardcoded 0.1.
    // Delegate to the ex variant with angular == chord == tolerance.
    return edge_approximation_segments_ex(edge, tolerance, tolerance);
}

std::unique_ptr<TopoDS_Edge> make_helix_edge(
    double ax, double ay, double az,
    double xrx, double xry, double xrz,
    double radius, double pitch, double height)
{
    try {
        if (radius < Precision::Confusion()) return nullptr;
        if (pitch < Precision::Confusion()) return nullptr;
        if (height < Precision::Confusion()) return nullptr;

        // Build a deterministic local frame: the cylinder's local +X is the
        // user-supplied x_ref (orthogonalized against axis by gp_Ax2). The
        // helix then starts at (radius, 0, 0) in this frame, which is
        // origin + radius * normalize(x_ref ⊥ axis) in world coordinates.
        gp_Dir axis_dir(ax, ay, az);
        gp_Dir x_ref(xrx, xry, xrz);
        if (axis_dir.IsParallel(x_ref, Precision::Angular())) return nullptr;
        gp_Ax2 ax2(gp_Pnt(0.0, 0.0, 0.0), axis_dir, x_ref);
        Handle(Geom_CylindricalSurface) cylinder =
            new Geom_CylindricalSurface(ax2, radius);

        double turns = height / pitch;
        double total_angle = turns * 2.0 * M_PI;
        gp_Pnt2d line_origin(0.0, 0.0);
        gp_Dir2d line_dir(total_angle, height);
        Handle(Geom2d_Line) line2d = new Geom2d_Line(line_origin, line_dir);

        double param_end = std::sqrt(total_angle * total_angle + height * height);

        BRepBuilderAPI_MakeEdge edgeMaker(line2d, cylinder, 0.0, param_end);
        if (!edgeMaker.IsDone()) return nullptr;
        TopoDS_Edge edge = edgeMaker.Edge();
        BRepLib::BuildCurve3d(edge);
        return std::make_unique<TopoDS_Edge>(edge);
    } catch (const Standard_Failure&) {
        return nullptr;
    }
}

std::unique_ptr<std::vector<TopoDS_Edge>> make_polygon_edges(rust::Slice<const double> coords) {
    auto out = std::make_unique<std::vector<TopoDS_Edge>>();
    if (coords.size() < 9 || coords.size() % 3 != 0) return out;
    try {
        BRepBuilderAPI_MakePolygon poly;
        for (size_t i = 0; i + 2 < coords.size(); i += 3) {
            poly.Add(gp_Pnt(coords[i], coords[i + 1], coords[i + 2]));
        }
        poly.Close();
        if (!poly.IsDone()) return out;
        TopoDS_Wire wire = poly.Wire();
        // Walk the wire's edges in order using TopExp_Explorer.
        for (TopExp_Explorer ex(wire, TopAbs_EDGE); ex.More(); ex.Next()) {
            out->push_back(TopoDS::Edge(ex.Current()));
        }
        return out;
    } catch (const Standard_Failure&) {
        out->clear();
        return out;
    }
}

std::unique_ptr<TopoDS_Edge> make_circle_edge(
    double ax, double ay, double az, double radius)
{
    try {
        if (radius < Precision::Confusion()) return nullptr;
        gp_Dir axis_dir(ax, ay, az);
        // Single-arg gp_Ax2(origin, N): OCCT picks an arbitrary X direction
        // orthogonal to the normal. The circle's parametric start is then at
        // (radius, 0, 0) in that implicit local frame. Callers that need a
        // specific start direction should rotate the result into place.
        gp_Ax2 ax2(gp_Pnt(0.0, 0.0, 0.0), axis_dir);
        gp_Circ circ(ax2, radius);
        BRepBuilderAPI_MakeEdge edgeMaker(circ);
        if (!edgeMaker.IsDone()) return nullptr;
        return std::make_unique<TopoDS_Edge>(edgeMaker.Edge());
    } catch (const Standard_Failure&) {
        return nullptr;
    }
}

std::unique_ptr<TopoDS_Edge> make_line_edge(
    double ax, double ay, double az,
    double bx, double by, double bz)
{
    try {
        gp_Pnt a(ax, ay, az);
        gp_Pnt b(bx, by, bz);
        if (a.Distance(b) < Precision::Confusion()) return nullptr;
        BRepBuilderAPI_MakeEdge edgeMaker(a, b);
        if (!edgeMaker.IsDone()) return nullptr;
        return std::make_unique<TopoDS_Edge>(edgeMaker.Edge());
    } catch (const Standard_Failure&) {
        return nullptr;
    }
}

std::unique_ptr<TopoDS_Edge> make_arc_edge(
    double sx, double sy, double sz,
    double mx, double my, double mz,
    double ex, double ey, double ez)
{
    try {
        gp_Pnt p_start(sx, sy, sz);
        gp_Pnt p_mid(mx, my, mz);
        gp_Pnt p_end(ex, ey, ez);
        // false: do not wrap around; the arc goes from start through
        // mid to end on the unique circle defined by those three points.
        GC_MakeArcOfCircle maker(p_start, p_mid, p_end);
        if (!maker.IsDone()) return nullptr;
        BRepBuilderAPI_MakeEdge edgeMaker(maker.Value());
        if (!edgeMaker.IsDone()) return nullptr;
        return std::make_unique<TopoDS_Edge>(edgeMaker.Edge());
    } catch (const Standard_Failure&) {
        return nullptr;
    }
}

// Cubic B-spline edge interpolating the given data points.
//
// `coords` is a flat array of xyz triples (length must be a multiple of 3
// and ≥ 6). Each (x, y, z) triple is one interpolation target — the
// resulting curve passes through every input point.
//
// `end_kind` selects the end-condition variant of `BSplineEnd`:
//   0 = Periodic — wraps around with C² continuity. Periodic is encoded
//       in the basis; the caller must NOT duplicate the first point at
//       the end. Needs ≥ 3 points (Rust side validates).
//   1 = NotAKnot — open curve, OCCT default end condition (the boundary
//       cubic is fit to 3 data points instead of being constrained by
//       an artificial derivative). Needs ≥ 2 points.
//   2 = Clamped — open curve with explicit start/end tangent vectors
//       passed in (sx, sy, sz) and (ex, ey, ez). Needs ≥ 2 points.
//
// For end_kind 0 and 1, the tangent arguments are ignored.
//
// Returns null on any failure (out-of-range end_kind, OCCT internal
// failure, degenerate point distribution).
std::unique_ptr<TopoDS_Edge> make_bspline_edge(
    rust::Slice<const double> coords,
    uint32_t end_kind,
    double sx, double sy, double sz,
    double ex, double ey, double ez)
{
    if (coords.size() < 6 || coords.size() % 3 != 0) return nullptr;
    try {
        // Local alias: `Handle(NCollection_HArray1<gp_Pnt>)` は Handle マクロが
        // template 内のカンマで引数を分割してしまうので、using alias を噛ませて
        // 単一トークン化する(コミット a72e330 で deprecated 型に戻した時の回避策)。
        using HPntArray = NCollection_HArray1<gp_Pnt>;
        const int n = static_cast<int>(coords.size() / 3);
        Handle(HPntArray) pts = new HPntArray(1, n);
        for (int i = 0; i < n; ++i) {
            pts->SetValue(i + 1, gp_Pnt(coords[i * 3], coords[i * 3 + 1], coords[i * 3 + 2]));
        }

        const bool periodic = (end_kind == 0) ? true : false;
        GeomAPI_Interpolate interp(pts, periodic, Precision::Confusion());

        if (end_kind == 2) {
            // Clamped: load explicit start and end tangent vectors.
            // Scale = true lets OCCT scale the tangent magnitude
            // by the chord length, which usually gives more intuitive
            // pull strength than the raw vector magnitude.
            gp_Vec start_tan(sx, sy, sz);
            gp_Vec end_tan(ex, ey, ez);
            interp.Load(start_tan, end_tan, true);
        } else if (end_kind != 0 && end_kind != 1) {
            // Unknown end_kind; fail rather than silently picking a default.
            return nullptr;
        }

        interp.Perform();
        if (!interp.IsDone()) return nullptr;

        Handle(Geom_BSplineCurve) curve = interp.Curve();
        if (curve.IsNull()) return nullptr;

        BRepBuilderAPI_MakeEdge edgeMaker(curve);
        if (!edgeMaker.IsDone()) return nullptr;
        return std::make_unique<TopoDS_Edge>(edgeMaker.Edge());
    } catch (const Standard_Failure&) {
        return nullptr;
    }
}

void edge_start_point(const TopoDS_Edge& edge, double& x, double& y, double& z) {
    x = 0.0; y = 0.0; z = 0.0;
    try {
        BRepAdaptor_Curve curve(edge);
        gp_Pnt p = curve.Value(curve.FirstParameter());
        x = p.X(); y = p.Y(); z = p.Z();
    } catch (const Standard_Failure&) {}
}

void edge_start_tangent(const TopoDS_Edge& edge, double& x, double& y, double& z) {
    x = 0.0; y = 0.0; z = 0.0;
    try {
        BRepAdaptor_Curve curve(edge);
        gp_Pnt p;
        gp_Vec v;
        curve.D1(curve.FirstParameter(), p, v);
        if (v.Magnitude() > Precision::Confusion()) {
            v.Normalize();
            x = v.X(); y = v.Y(); z = v.Z();
        }
    } catch (const Standard_Failure&) {}
}

bool edge_is_closed(const TopoDS_Edge& edge) {
    try {
        BRepAdaptor_Curve curve(edge);
        gp_Pnt p_start = curve.Value(curve.FirstParameter());
        gp_Pnt p_end   = curve.Value(curve.LastParameter());
        return p_start.Distance(p_end) < Precision::Confusion();
    } catch (const Standard_Failure&) {
        return false;
    }
}

std::unique_ptr<TopoDS_Edge> deep_copy_edge(const TopoDS_Edge& edge) {
    try {
        BRepBuilderAPI_Copy copier(edge);
        return std::make_unique<TopoDS_Edge>(TopoDS::Edge(copier.Shape()));
    } catch (const Standard_Failure&) {
        return nullptr;
    }
}

// Helper: apply a gp_Trsf to an edge via BRepBuilderAPI_Transform.
// Used for all four edge transforms below.
static std::unique_ptr<TopoDS_Edge> transform_edge_impl(
    const TopoDS_Edge& edge, const gp_Trsf& trsf)
{
    try {
        BRepBuilderAPI_Transform transform(edge, trsf, true);
        return std::make_unique<TopoDS_Edge>(TopoDS::Edge(transform.Shape()));
    } catch (const Standard_Failure&) {
        return nullptr;
    }
}

std::unique_ptr<TopoDS_Edge> translate_edge(
    const TopoDS_Edge& edge, double tx, double ty, double tz)
{
    gp_Trsf trsf;
    trsf.SetTranslation(gp_Vec(tx, ty, tz));
    return transform_edge_impl(edge, trsf);
}

std::unique_ptr<TopoDS_Edge> rotate_edge(
    const TopoDS_Edge& edge,
    double ox, double oy, double oz,
    double dx, double dy, double dz,
    double angle)
{
    try {
        gp_Trsf trsf;
        trsf.SetRotation(gp_Ax1(gp_Pnt(ox, oy, oz), gp_Dir(dx, dy, dz)), angle);
        return transform_edge_impl(edge, trsf);
    } catch (const Standard_Failure&) {
        return nullptr;
    }
}

std::unique_ptr<TopoDS_Edge> scale_edge(
    const TopoDS_Edge& edge,
    double cx, double cy, double cz,
    double factor)
{
    gp_Trsf trsf;
    trsf.SetScale(gp_Pnt(cx, cy, cz), factor);
    return transform_edge_impl(edge, trsf);
}

std::unique_ptr<TopoDS_Edge> mirror_edge(
    const TopoDS_Edge& edge,
    double ox, double oy, double oz,
    double nx, double ny, double nz)
{
    try {
        gp_Trsf trsf;
        trsf.SetMirror(gp_Ax2(gp_Pnt(ox, oy, oz), gp_Dir(nx, ny, nz)));
        return transform_edge_impl(edge, trsf);
    } catch (const Standard_Failure&) {
        return nullptr;
    }
}

std::unique_ptr<std::vector<TopoDS_Edge>> edge_vec_new() {
    return std::make_unique<std::vector<TopoDS_Edge>>();
}

void edge_vec_push(std::vector<TopoDS_Edge>& v, const TopoDS_Edge& e) {
    v.push_back(e);
}

void edge_vec_push_null(std::vector<TopoDS_Edge>& v) {
    v.push_back(TopoDS_Edge());
}

// Extrude a closed profile wire into a solid via BRepPrimAPI_MakePrism.
// Edges → Wire → Face → Prism (solid).
std::unique_ptr<TopoDS_Shape> make_extrude(
    const std::vector<TopoDS_Edge>& profile_edges,
    double dx, double dy, double dz)
{
    try {
        if (profile_edges.empty()) return nullptr;
        BRepBuilderAPI_MakeWire wire_maker;
        for (const auto& e : profile_edges) wire_maker.Add(e);
        if (!wire_maker.IsDone()) return nullptr;
        BRepBuilderAPI_MakeFace face_maker(wire_maker.Wire());
        if (!face_maker.IsDone()) return nullptr;
        gp_Vec dir(dx, dy, dz);
        BRepPrimAPI_MakePrism prism(face_maker.Face(), dir);
        prism.Build();
        if (!prism.IsDone()) return nullptr;
        return std::make_unique<TopoDS_Shape>(prism.Shape());
    } catch (const Standard_Failure&) {
        return nullptr;
    }
}

// Unified MakePipeShell wrapper.  Handles both single-profile sweep and
// multi-profile morphing sweep.  Profile sections in `all_edges` are
// separated by null-edge sentinels (TopoDS_Edge().IsNull() == true).
// `aux_spine_edges` is used only when orient == 3 (Auxiliary); pass an
// empty vector for other modes.
std::unique_ptr<TopoDS_Shape> make_pipe_shell(
    const std::vector<TopoDS_Edge>& all_edges,
    const std::vector<TopoDS_Edge>& spine_edges,
    uint32_t orient,
    double ux, double uy, double uz,
    const std::vector<TopoDS_Edge>& aux_spine_edges)
{
    try {
        if (all_edges.empty() || spine_edges.empty()) return nullptr;

        // Build the spine wire.
        BRepBuilderAPI_MakeWire spineMaker;
        for (const auto& e : spine_edges) spineMaker.Add(e);
        if (!spineMaker.IsDone()) return nullptr;
        TopoDS_Wire spine = spineMaker.Wire();

        BRepOffsetAPI_MakePipeShell shell(spine);

        // Configure trihedron law.
        switch (orient) {
            case 0: {
                // Fixed: lock the trihedron to the spine-start frame.
                BRepAdaptor_Curve curve(spine_edges.front());
                gp_Pnt start_pnt;
                gp_Vec start_tan;
                curve.D1(curve.FirstParameter(), start_pnt, start_tan);
                if (start_tan.Magnitude() < Precision::Confusion()) return nullptr;
                gp_Dir tdir(start_tan);
                gp_Dir xref = (std::abs(tdir.X()) < 0.9) ? gp_Dir(1, 0, 0) : gp_Dir(0, 1, 0);
                gp_Ax2 fixed_ax2(start_pnt, tdir, xref);
                shell.SetMode(fixed_ax2);
                break;
            }
            case 1: {
                // Torsion: raw Frenet.
                shell.SetMode(true);
                break;
            }
            case 2: {
                // Up(v): fix the binormal direction.
                gp_Vec up_vec(ux, uy, uz);
                if (up_vec.Magnitude() < Precision::Confusion()) return nullptr;
                shell.SetMode(gp_Dir(up_vec));
                break;
            }
            case 3: {
                // Auxiliary: build aux spine wire and use it for twist control.
                if (aux_spine_edges.empty()) return nullptr;
                BRepBuilderAPI_MakeWire auxMaker;
                for (const auto& e : aux_spine_edges) auxMaker.Add(e);
                if (!auxMaker.IsDone()) return nullptr;
                shell.SetMode(auxMaker.Wire(), false);
                break;
            }
            default: {
                shell.SetMode(true);
                break;
            }
        }

        // Split all_edges by null sentinels into profile wires and Add each.
        BRepBuilderAPI_MakeWire wire_maker;
        bool has_edges = false;
        for (const auto& e : all_edges) {
            if (e.IsNull()) {
                if (!wire_maker.IsDone()) return nullptr;
                shell.Add(wire_maker.Wire(), false, false);
                wire_maker = BRepBuilderAPI_MakeWire();
                has_edges = false;
            } else {
                wire_maker.Add(e);
                has_edges = true;
            }
        }
        // Last section (after final sentinel or single section with no sentinel).
        if (has_edges) {
            if (!wire_maker.IsDone()) return nullptr;
            shell.Add(wire_maker.Wire(), false, false);
        }

        shell.Build();
        if (!shell.IsDone()) return nullptr;
        if (!shell.MakeSolid()) return nullptr;
        return std::make_unique<TopoDS_Shape>(shell.Shape());
    } catch (const Standard_Failure&) {
        return nullptr;
    }
}

// Loft (skin) a smooth solid through a sequence of cross-section wires.
//
// `all_edges` is a flattened list of all edges across all sections; the
// `section_sizes` array tells how many edges belong to each section. Example:
//   sections [[a, b, c], [d, e], [f, g, h, i]]
//   → all_edges = [a, b, c, d, e, f, g, h, i]
//     section_sizes = [3, 2, 4]
//
// When `closed == true`, the first section's TopoDS_Wire is reused (NOT
// copied) as the last section. OCCT's BRepOffsetAPI_ThruSections checks
// `myWires(1).IsSame(myWires(nbSects))` (TShape* pointer identity) and
// switches to a v-direction periodic surface internally — see
// BRepOffsetAPI_ThruSections.cxx lines 539, 691, and 1187-1189. The
// resulting surface is C² continuous across the wrap-around because the
// underlying GeomFill_AppSurf processes all sections at once with periodic
// boundary conditions. Crucially we must NOT BRepBuilderAPI_Copy the wire
// — that would assign a fresh TShape* and the IsSame() check would fail,
// silently degrading to an open loft.
//
// `isSolid=true` requests OCCT cap the open ends with planar faces (when
// `closed=false`); `isRuled=false` requests B-spline (smoothed) interpolation
// rather than panel-by-panel ruled surfaces — necessary for the C² guarantee.
// Loft (skin) through cross-section wires.  Sections in `all_edges` are
// separated by null-edge sentinels.
std::unique_ptr<TopoDS_Shape> make_loft(
    const std::vector<TopoDS_Edge>& all_edges)
{
    try {
        BRepOffsetAPI_ThruSections loft(
            /*isSolid=*/true,
            /*isRuled=*/false,
            Precision::Confusion());

        // Split all_edges by null sentinels into section wires.
        size_t wire_count = 0;
        BRepBuilderAPI_MakeWire wire_maker;
        bool has_edges = false;

        auto flush_wire = [&]() -> bool {
            if (!has_edges) return true;
            if (!wire_maker.IsDone()) return false;
            loft.AddWire(wire_maker.Wire());
            wire_count++;
            wire_maker = BRepBuilderAPI_MakeWire();
            has_edges = false;
            return true;
        };

        for (const auto& e : all_edges) {
            if (e.IsNull()) {
                if (!flush_wire()) return nullptr;
            } else {
                wire_maker.Add(e);
                has_edges = true;
            }
        }
        if (!flush_wire()) return nullptr;

        if (wire_count < 2) return nullptr;

        loft.Build();
        if (!loft.IsDone()) return nullptr;
        return std::make_unique<TopoDS_Shape>(loft.Shape());
    } catch (const Standard_Failure&) {
        return nullptr;
    }
}

std::unique_ptr<TopoDS_Shape> make_bspline_solid(
    rust::Slice<const double> coords,
    uint32_t nu, uint32_t nv,
    bool u_periodic)
{
    try {
        if (coords.size() != static_cast<size_t>(nu) * nv * 3) return nullptr;
        if (nu < 2 || nv < 3) return nullptr;

        // Augment for periodicity: duplicate first row/col at the end so that
        // the grid is geometrically closed in the periodic direction(s).
        // V is ALWAYS periodic (closed cross-section); U only when u_periodic.
        const uint32_t u_extra = u_periodic ? 1u : 0u;
        const uint32_t v_extra = 1u;
        const uint32_t total_u = nu + u_extra;
        const uint32_t total_v = nv + v_extra;

        NCollection_Array2<gp_Pnt> pts(1, static_cast<int>(total_u), 1, static_cast<int>(total_v));
        for (uint32_t i = 0; i < total_u; ++i) {
            const uint32_t src_i = (i >= nu) ? 0u : i;
            for (uint32_t j = 0; j < total_v; ++j) {
                const uint32_t src_j = (j >= nv) ? 0u : j;
                const size_t idx = (static_cast<size_t>(src_i) * nv + src_j) * 3;
                pts.SetValue(
                    static_cast<int>(i) + 1,
                    static_cast<int>(j) + 1,
                    gp_Pnt(coords[idx], coords[idx + 1], coords[idx + 2]));
            }
        }

        // Interpolate a B-spline surface through the augmented grid.
        Handle(Geom_BSplineSurface) surface;
        try {
            GeomAPI_PointsToBSplineSurface fitter;
            fitter.Interpolate(pts);
            if (!fitter.IsDone()) return nullptr;
            surface = fitter.Surface();
        } catch (const Standard_Failure&) {
            return nullptr;
        }
        if (surface.IsNull()) return nullptr;

        // Promote geometric closure to B-spline periodicity.
        // SetVPeriodic/SetUPeriodic require pole rows/cols to match within
        // tolerance — the augmentation above ensures that.
        try {
            surface->SetVPeriodic();
        } catch (const Standard_Failure&) {
            // Fall through: non-periodic V; sewing may still close it.
        }
        if (u_periodic) {
            try {
                surface->SetUPeriodic();
            } catch (const Standard_Failure&) {
                // Fall through.
            }
        }

        // Side face spans the full parametric domain.
        double u1, u2, v1, v2;
        surface->Bounds(u1, u2, v1, v2);
        BRepBuilderAPI_MakeFace face_maker(surface, Precision::Confusion());
        if (!face_maker.IsDone()) return nullptr;
        TopoDS_Face side_face = face_maker.Face();

        BRepBuilderAPI_Sewing sewing(1.0e-3);
        sewing.Add(side_face);

        // For non-periodic U, cap the two U-boundary loops with planar faces.
        // For periodic U the surface is already closed into a torus — no caps.
        if (!u_periodic) {
            auto make_cap = [&](double u_at) -> TopoDS_Face {
                Handle(Geom_Curve) iso = surface->UIso(u_at);
                if (iso.IsNull()) return TopoDS_Face();
                BRepBuilderAPI_MakeEdge em(iso, v1, v2);
                if (!em.IsDone()) return TopoDS_Face();
                BRepBuilderAPI_MakeWire wm(em.Edge());
                if (!wm.IsDone()) return TopoDS_Face();
                BRepBuilderAPI_MakeFace mf(wm.Wire(), true);
                return mf.IsDone() ? mf.Face() : TopoDS_Face();
            };
            TopoDS_Face cap1 = make_cap(u1);
            TopoDS_Face cap2 = make_cap(u2);
            if (cap1.IsNull() || cap2.IsNull()) return nullptr;
            sewing.Add(cap1);
            sewing.Add(cap2);
        }

        sewing.Perform();
        TopoDS_Shape sewn = sewing.SewedShape();
        if (sewn.IsNull()) return nullptr;

        TopoDS_Shell shell;
        if (sewn.ShapeType() == TopAbs_SHELL) {
            shell = TopoDS::Shell(sewn);
        } else if (sewn.ShapeType() == TopAbs_SOLID) {
            return std::make_unique<TopoDS_Shape>(sewn);
        } else if (sewn.ShapeType() == TopAbs_FACE && u_periodic) {
            // Full torus: single closed face → wrap manually.
            BRep_Builder bb;
            bb.MakeShell(shell);
            bb.Add(shell, TopoDS::Face(sewn));
            shell.Closed(true);
        } else {
            TopExp_Explorer exp(sewn, TopAbs_SHELL);
            if (exp.More()) {
                shell = TopoDS::Shell(exp.Current());
            } else {
                return nullptr;
            }
        }

        BRepBuilderAPI_MakeSolid solid_maker(shell);
        if (!solid_maker.IsDone()) return nullptr;
        TopoDS_Solid solid = solid_maker.Solid();

        // Ensure outward-facing orientation.
        BRepClass3d_SolidClassifier classifier(
            solid, gp_Pnt(0, 0, 0), Precision::Confusion());
        if (classifier.State() == TopAbs_IN) {
            solid.Reverse();
        }

        return std::make_unique<TopoDS_Shape>(solid);
    } catch (const Standard_Failure&) {
        return nullptr;
    }
}

} // namespace cadrum

#ifdef CADRUM_COLOR

#include <XCAFDoc_DocumentTool.hxx>
#include <XCAFDoc_ShapeTool.hxx>
#include <XCAFDoc_ColorTool.hxx>
#include <STEPCAFControl_Reader.hxx>
#include <STEPCAFControl_Writer.hxx>
#include <TDocStd_Document.hxx>
#include <TDF_ChildIterator.hxx>
#include <NCollection_Sequence.hxx>
#include <TDF_Label.hxx>
#include <Quantity_Color.hxx>

namespace cadrum {

// Traverse every label in the XDE document and record face-level colors.
// Uses TDF_ChildIterator with allLevels=true for a flat, efficient walk.
static void collect_face_colors(
    const Handle(TDocStd_Document)& doc,
    const Handle(XCAFDoc_ColorTool)& colorTool,
    std::unordered_map<uint64_t, std::array<float, 3>>& colorMap)
{
    for (TDF_ChildIterator it(doc->Main(), true); it.More(); it.Next()) {
        const TDF_Label& label = it.Value();
        if (!XCAFDoc_ShapeTool::IsShape(label)) continue;

        TopoDS_Shape s = XCAFDoc_ShapeTool::GetShape(label);
        if (s.IsNull() || s.ShapeType() != TopAbs_FACE) continue;

        Quantity_Color color;
        bool ok = colorTool->GetColor(label, XCAFDoc_ColorSurf, color);
        if (!ok) ok = colorTool->GetColor(label, XCAFDoc_ColorGen, color);
        if (!ok) continue;

        uint64_t id = reinterpret_cast<uint64_t>(s.TShape().get());
        colorMap[id] = {(float)color.Red(), (float)color.Green(), (float)color.Blue()};
    }
}

std::unique_ptr<ColoredStepData> read_step_color_stream(RustReader& reader) {
    try {
        // Create XDE document directly — avoids XCAFApp_Application which
        // pulls in visualization libs (TKXCAFPrs/TKTPrsStd) built with
        // BUILD_MODULE_Visualization=OFF.  Handle<> ref-counts ownership.
        Handle(TDocStd_Document) doc = new TDocStd_Document("XmlXCAF");

        STEPCAFControl_Reader cafreader;
        cafreader.SetColorMode(true);

        RustReadStreambuf sbuf(reader);
        std::istream is(&sbuf);
        if (cafreader.ReadStream("stream", is) != IFSelect_RetDone) {
            return nullptr;
        }
        if (!cafreader.Transfer(doc)) {
            return nullptr;
        }

        Handle(XCAFDoc_ShapeTool) shapeTool =
            XCAFDoc_DocumentTool::ShapeTool(doc->Main());
        Handle(XCAFDoc_ColorTool) colorTool =
            XCAFDoc_DocumentTool::ColorTool(doc->Main());

        // Collect all free shapes into a compound.
        NCollection_Sequence<TDF_Label> roots;
        shapeTool->GetFreeShapes(roots);

        BRep_Builder builder;
        TopoDS_Compound compound;
        builder.MakeCompound(compound);
        for (int i = 1; i <= roots.Length(); i++) {
            builder.Add(compound, shapeTool->GetShape(roots.Value(i)));
        }

        // Build TShape* → color map from the XDE document labels.
        std::unordered_map<uint64_t, std::array<float, 3>> colorMap;
        collect_face_colors(doc, colorTool, colorMap);

        auto result = std::make_unique<ColoredStepData>();
        result->shape = compound;

        // Emit colors for each face that has a color entry.
        for (TopExp_Explorer ex(compound, TopAbs_FACE); ex.More(); ex.Next()) {
            uint64_t id =
                reinterpret_cast<uint64_t>(ex.Current().TShape().get());
            auto it = colorMap.find(id);
            if (it == colorMap.end()) continue;
            result->ids.push_back(id);
            result->r.push_back(it->second[0]);
            result->g.push_back(it->second[1]);
            result->b.push_back(it->second[2]);
        }

        return result;
    } catch (const Standard_Failure&) {
        return nullptr;
    }
}

std::unique_ptr<TopoDS_Shape> colored_step_shape(const ColoredStepData& d) {
    return std::make_unique<TopoDS_Shape>(d.shape);
}

rust::Vec<uint64_t> colored_step_ids(const ColoredStepData& d) {
    rust::Vec<uint64_t> v;
    for (uint64_t x : d.ids) v.push_back(x);
    return v;
}

rust::Vec<float> colored_step_colors_r(const ColoredStepData& d) {
    rust::Vec<float> v;
    for (float x : d.r) v.push_back(x);
    return v;
}

rust::Vec<float> colored_step_colors_g(const ColoredStepData& d) {
    rust::Vec<float> v;
    for (float x : d.g) v.push_back(x);
    return v;
}

rust::Vec<float> colored_step_colors_b(const ColoredStepData& d) {
    rust::Vec<float> v;
    for (float x : d.b) v.push_back(x);
    return v;
}

bool write_step_color_stream(
    const TopoDS_Shape&         shape,
    rust::Slice<const uint64_t> ids,
    rust::Slice<const float>    cr,
    rust::Slice<const float>    cg,
    rust::Slice<const float>    cb,
    RustWriter&                 writer)
{
    try {
        Handle(TDocStd_Document) doc = new TDocStd_Document("XmlXCAF");

        Handle(XCAFDoc_ShapeTool) shapeTool =
            XCAFDoc_DocumentTool::ShapeTool(doc->Main());
        Handle(XCAFDoc_ColorTool) colorTool =
            XCAFDoc_DocumentTool::ColorTool(doc->Main());

        // Register the root shape.
        TDF_Label rootLabel = shapeTool->AddShape(shape, false);

        // Build TShape* → color lookup from the Rust-supplied arrays.
        std::unordered_map<uint64_t, std::array<float, 3>> colorLookup;
        for (size_t i = 0; i < ids.size(); i++) {
            colorLookup[ids[i]] = {cr[i], cg[i], cb[i]};
        }

        // For each colored face, find/create its sub-shape label and set color.
        for (TopExp_Explorer ex(shape, TopAbs_FACE); ex.More(); ex.Next()) {
            const TopoDS_Shape& face = ex.Current();
            uint64_t id = reinterpret_cast<uint64_t>(face.TShape().get());
            auto it = colorLookup.find(id);
            if (it == colorLookup.end()) continue;

            TDF_Label faceLabel;
            if (!shapeTool->FindSubShape(rootLabel, face, faceLabel)) {
                faceLabel = shapeTool->AddSubShape(rootLabel, face);
            }

            const auto& c = it->second;
            Quantity_Color color(c[0], c[1], c[2], Quantity_TOC_RGB);
            colorTool->SetColor(faceLabel, color, XCAFDoc_ColorSurf);
        }

        // Transfer XDE doc to STEP model and write to stream.
        STEPCAFControl_Writer cafwriter;
        cafwriter.SetColorMode(true);
        if (!cafwriter.Transfer(doc)) {
            return false;
        }

        RustWriteStreambuf sbuf(writer);
        std::ostream os(&sbuf);
        return cafwriter.ChangeWriter().WriteStream(os) == IFSelect_RetDone;
    } catch (const Standard_Failure&) {
        return false;
    }
}

// ==================== Clean with face-origin mapping ====================

std::unique_ptr<CleanShape> clean_shape_full(const TopoDS_Shape& shape) {
    try {
        ShapeUpgrade_UnifySameDomain unifier(shape, true, true, true);
        unifier.AllowInternalEdges(false);
        unifier.Build();

        auto r = std::make_unique<CleanShape>();
        r->shape = unifier.Shape();

        Handle(BRepTools_History) history = unifier.History();
        if (!history.IsNull()) {
            for (TopExp_Explorer ex(shape, TopAbs_FACE); ex.More(); ex.Next()) {
                const TopoDS_Shape& old_face = ex.Current();
                uint64_t old_id = reinterpret_cast<uint64_t>(old_face.TShape().get());
                if (history->IsRemoved(old_face)) continue;
                const NCollection_List<TopoDS_Shape>& mods = history->Modified(old_face);
                if (mods.IsEmpty()) {
                    // Unchanged: TShape* is the same in the result.
                    r->mapping.push_back(old_id);
                    r->mapping.push_back(old_id);
                } else {
                    // Merged: use only the first resulting face (first-found wins).
                    uint64_t new_id = reinterpret_cast<uint64_t>(mods.First().TShape().get());
                    r->mapping.push_back(new_id);
                    r->mapping.push_back(old_id);
                }
            }
        }
        return r;
    } catch (const Standard_Failure&) {
        return nullptr;
    }
}

std::unique_ptr<TopoDS_Shape> clean_shape_get(const CleanShape& r) {
    return std::make_unique<TopoDS_Shape>(r.shape);
}

rust::Vec<uint64_t> clean_shape_mapping(const CleanShape& r) {
    rust::Vec<uint64_t> v;
    for (uint64_t x : r.mapping) v.push_back(x);
    return v;
}

} // namespace cadrum

#endif // CADRUM_COLOR

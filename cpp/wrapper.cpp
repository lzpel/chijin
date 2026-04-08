#include "cadrum/src/occt/ffi.rs.h"

// Implementation-only OCCT headers (not exposed via wrapper.h)
#include <Standard_Failure.hxx>
#include <TopoDS_Solid.hxx>
#include <TopoDS_Compound.hxx>
#include <TopAbs_ShapeEnum.hxx>
#include <TopoDS.hxx>

#include <BRepBuilderAPI_Copy.hxx>
#include <BRepBuilderAPI_MakeFace.hxx>
#include <BRepBuilderAPI_MakePolygon.hxx>
#include <BRepBuilderAPI_Transform.hxx>
#include <BRepPrimAPI_MakeBox.hxx>
#include <BRepPrimAPI_MakeCone.hxx>
#include <BRepPrimAPI_MakeCylinder.hxx>
#include <BRepPrimAPI_MakeHalfSpace.hxx>
#include <BRepPrimAPI_MakeSphere.hxx>
#include <BRepPrimAPI_MakeTorus.hxx>
#include <BRepPrimAPI_MakePrism.hxx>
#include <BRepPrimAPI_MakeRevol.hxx>
#include <BRepOffsetAPI_MakePipeShell.hxx>
#include <BRepBuilderAPI_MakeEdge.hxx>
#include <BRepBuilderAPI_MakeWire.hxx>
#include <Geom_CylindricalSurface.hxx>
#include <Geom2d_Line.hxx>
#include <BRepTools.hxx>
#include <BRepLib.hxx>
#include <gp_Ax1.hxx>
#include <gp_Ax2.hxx>

#include <BRepAlgoAPI_BooleanOperation.hxx>
#include <BRepAlgoAPI_Fuse.hxx>
#include <BRepAlgoAPI_Cut.hxx>
#include <BRepAlgoAPI_Common.hxx>

#include <ShapeUpgrade_UnifySameDomain.hxx>
#include <BRepTools_History.hxx>

#include <BRepMesh_IncrementalMesh.hxx>
#include <BRep_Tool.hxx>
#include <Poly_Triangulation.hxx>
#include <BRepClass3d_SolidClassifier.hxx>
#include <BRepBndLib.hxx>
#include <Bnd_Box.hxx>
#include <BRepGProp.hxx>
#include <BRepGProp_Face.hxx>
#include <GProp_GProps.hxx>
#include <GeomAPI_ProjectPointOnSurf.hxx>

#include <BRepAdaptor_Curve.hxx>
#include <GCPnts_TangentialDeflection.hxx>

#include <BRep_Builder.hxx>
#include <TopExp.hxx>
#include <TopTools_IndexedMapOfShape.hxx>
#include <TopTools_ListOfShape.hxx>
#include <gp_Pln.hxx>
#include <gp_Ax2.hxx>
#include <gp_Trsf.hxx>
#include <TopLoc_Location.hxx>

#include <BinTools.hxx>
#include <BRepTools.hxx>
#include <STEPControl_Reader.hxx>
#include <STEPControl_Writer.hxx>
#include <Message_ProgressRange.hxx>

#include <istream>
#include <ostream>
#include <sstream>
#include <cmath>
#include <cstring>
#include <algorithm>
#include <cstdint>
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

// ==================== Shape I/O (streambuf callback) ====================

std::unique_ptr<TopoDS_Shape> read_step_stream(RustReader& reader) {
    RustReadStreambuf sbuf(reader);
    std::istream is(&sbuf);

    // Allocate reader on the heap and leak it (Bug 2 fix).
    auto* step_reader = new STEPControl_Reader();
    IFSelect_ReturnStatus status = step_reader->ReadStream("stream", is);

    if (status != IFSelect_RetDone) {
        return nullptr;
    }

    step_reader->TransferRoots(Message_ProgressRange());
    return std::make_unique<TopoDS_Shape>(step_reader->OneShape());
    // Intentionally leak step_reader to prevent destructor crash.
}

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
    BRepBuilderAPI_Copy copier(shape, Standard_True, Standard_False);
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
        const TopTools_ListOfShape& mods = op.Modified(sf);
        if (mods.IsEmpty()) {
            // Face is unchanged: its TShape* appears as-is in op.Shape().
            relay[src_id] = src_id;
        } else {
            for (TopTools_ListOfShape::Iterator it(mods); it.More(); it.Next()) {
                uint64_t pre_id = reinterpret_cast<uint64_t>(it.Value().TShape().get());
                relay[pre_id] = src_id;
            }
        }
    }
}

// Helper: after BRepBuilderAPI_Copy, match pre/post faces by their index in
// TopTools_IndexedMapOfShape (BRepBuilderAPI_Copy preserves traversal order).
// Emit [post_id, src_id] pairs into `out` for every face tracked in `relay`.
static void emit_from_pairs(
    const TopoDS_Shape& pre_shape,
    const TopoDS_Shape& post_shape,
    const std::unordered_map<uint64_t, uint64_t>& relay,
    std::vector<uint64_t>& out)
{
    TopTools_IndexedMapOfShape pre_map, post_map;
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

std::unique_ptr<BooleanShape> boolean_fuse(
    const TopoDS_Shape& a, const TopoDS_Shape& b)
{
    try {
        BRepAlgoAPI_Fuse fuse(a, b);
        fuse.Build();
        if (!fuse.IsDone()) return nullptr;

        std::unordered_map<uint64_t, uint64_t> relay_a, relay_b;
        collect_relay_mapping(fuse, a, relay_a);
        collect_relay_mapping(fuse, b, relay_b);

        BRepBuilderAPI_Copy copier(fuse.Shape(), Standard_True, Standard_False);
        auto r = std::make_unique<BooleanShape>();
        r->shape = copier.Shape();
        emit_from_pairs(fuse.Shape(), copier.Shape(), relay_a, r->from_a);
        emit_from_pairs(fuse.Shape(), copier.Shape(), relay_b, r->from_b);
        return r;
    } catch (const Standard_Failure&) {
        return nullptr;
    }
}

std::unique_ptr<BooleanShape> boolean_cut(
    const TopoDS_Shape& a, const TopoDS_Shape& b)
{
    try {
        BRepAlgoAPI_Cut cut(a, b);
        cut.Build();
        if (!cut.IsDone()) return nullptr;

        std::unordered_map<uint64_t, uint64_t> relay_a, relay_b;
        collect_relay_mapping(cut, a, relay_a);
        collect_relay_mapping(cut, b, relay_b);

        BRepBuilderAPI_Copy copier(cut.Shape(), Standard_True, Standard_False);
        auto r = std::make_unique<BooleanShape>();
        r->shape = copier.Shape();
        emit_from_pairs(cut.Shape(), copier.Shape(), relay_a, r->from_a);
        emit_from_pairs(cut.Shape(), copier.Shape(), relay_b, r->from_b);
        return r;
    } catch (const Standard_Failure&) {
        return nullptr;
    }
}

std::unique_ptr<BooleanShape> boolean_common(
    const TopoDS_Shape& a, const TopoDS_Shape& b)
{
    try {
        BRepAlgoAPI_Common common(a, b);
        common.Build();
        if (!common.IsDone()) return nullptr;

        std::unordered_map<uint64_t, uint64_t> relay_a, relay_b;
        collect_relay_mapping(common, a, relay_a);
        collect_relay_mapping(common, b, relay_b);

        BRepBuilderAPI_Copy copier(common.Shape(), Standard_True, Standard_False);
        auto r = std::make_unique<BooleanShape>();
        r->shape = copier.Shape();
        emit_from_pairs(common.Shape(), copier.Shape(), relay_a, r->from_a);
        emit_from_pairs(common.Shape(), copier.Shape(), relay_b, r->from_b);
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

std::unique_ptr<TopoDS_Shape> clean_shape(const TopoDS_Shape& shape) {
    try {
        ShapeUpgrade_UnifySameDomain unifier(shape, Standard_True, Standard_True, Standard_True);
        unifier.AllowInternalEdges(Standard_False);
        unifier.Build();
        return std::make_unique<TopoDS_Shape>(unifier.Shape());
    } catch (const Standard_Failure&) {
        return nullptr;
    }
}

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
        BRepBuilderAPI_Transform transform(shape, trsf, Standard_True);
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
        BRepBuilderAPI_Transform transform(shape, trsf, Standard_True);
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
        BRepGProp_Face prop_face(face);

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
    TopTools_IndexedMapOfShape edgeMap;
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

void face_center_of_mass(const TopoDS_Face& face,
    double& cx, double& cy, double& cz)
{
    try {
        GProp_GProps props;
        BRepGProp::SurfaceProperties(face, props);
        gp_Pnt center = props.CentreOfMass();
        cx = center.X();
        cy = center.Y();
        cz = center.Z();
    } catch (const Standard_Failure&) {
        cx = cy = cz = 0.0;
    }
}

void face_normal_at_center(const TopoDS_Face& face,
    double& nx, double& ny, double& nz)
{
    try {
        // Step 1: Get center of mass
        GProp_GProps props;
        BRepGProp::SurfaceProperties(face, props);
        gp_Pnt center = props.CentreOfMass();

        // Step 2: Get surface and project center point onto it
        Handle(Geom_Surface) surface = BRep_Tool::Surface(face);
        GeomAPI_ProjectPointOnSurf projector(center, surface);

        // LowerDistanceParameters throws StdFail_NotDone when NbPoints == 0
        if (projector.NbPoints() == 0) {
            nx = ny = nz = 0.0;
            return;
        }

        double u, v;
        projector.LowerDistanceParameters(u, v);

        // Step 3: Get normal at (u, v)
        BRepGProp_Face gprop_face(face);
        gp_Pnt point;
        gp_Vec normal;
        gprop_face.Normal(u, v, point, normal);

        if (normal.Magnitude() > 1e-10) {
            normal.Normalize();
        }

        nx = normal.X();
        ny = normal.Y();
        nz = normal.Z();
    } catch (const Standard_Failure&) {
        nx = ny = nz = 0.0;
    }
}

std::unique_ptr<TopoDS_Shape> face_extrude(const TopoDS_Face& face,
    double dx, double dy, double dz)
{
    try {
        gp_Vec prism_vec(dx, dy, dz);
        BRepPrimAPI_MakePrism maker(face, prism_vec, Standard_False, Standard_True);
        return std::make_unique<TopoDS_Shape>(maker.Shape());
    } catch (const Standard_Failure&) {
        return nullptr;
    }
}

std::unique_ptr<TopoDS_Face> face_from_polygon(rust::Slice<const double> coords)
{
    if (coords.size() < 9 || coords.size() % 3 != 0) return nullptr;
    try {
        BRepBuilderAPI_MakePolygon poly;
        for (size_t i = 0; i + 2 < coords.size(); i += 3) {
            poly.Add(gp_Pnt(coords[i], coords[i + 1], coords[i + 2]));
        }
        poly.Close();
        if (!poly.IsDone()) return nullptr;
        BRepBuilderAPI_MakeFace face_maker(poly.Wire(), Standard_True);
        if (!face_maker.IsDone()) return nullptr;
        return std::make_unique<TopoDS_Face>(face_maker.Face());
    } catch (const Standard_Failure&) {
        return nullptr;
    }
}

std::unique_ptr<TopoDS_Shape> face_revolve(const TopoDS_Face& face,
    double ox, double oy, double oz,
    double dx, double dy, double dz,
    double angle)
{
    try {
        gp_Ax1 axis(gp_Pnt(ox, oy, oz), gp_Dir(dx, dy, dz));
        BRepPrimAPI_MakeRevol maker(face, axis, angle);
        if (!maker.IsDone()) return nullptr;
        return std::make_unique<TopoDS_Shape>(maker.Shape());
    } catch (const Standard_Failure&) {
        return nullptr;
    }
}

std::unique_ptr<TopoDS_Shape> face_helix(const TopoDS_Face& face,
    double ox, double oy, double oz,
    double dx, double dy, double dz,
    double pitch, double turns, bool align_to_spine)
{
    try {
        // Compute radius from face centroid to axis
        GProp_GProps props;
        BRepGProp::SurfaceProperties(face, props);
        gp_Pnt centroid = props.CentreOfMass();

        gp_Pnt origin(ox, oy, oz);
        gp_Dir dir(dx, dy, dz);
        gp_Lin axis_line(origin, dir);
        double radius = axis_line.Distance(centroid);
        if (radius < Precision::Confusion()) return nullptr;

        // Build helix spine on a cylindrical surface
        gp_Ax2 ax2(origin, dir);
        Handle(Geom_CylindricalSurface) cylinder =
            new Geom_CylindricalSurface(ax2, radius);

        double total_angle = turns * 2.0 * M_PI;
        double height = pitch * turns;
        gp_Pnt2d line_origin(0.0, 0.0);
        gp_Dir2d line_dir(total_angle, height);
        Handle(Geom2d_Line) line2d = new Geom2d_Line(line_origin, line_dir);

        double param_end = std::sqrt(total_angle * total_angle + height * height);

        BRepBuilderAPI_MakeEdge edgeMaker(line2d, cylinder, 0.0, param_end);
        if (!edgeMaker.IsDone()) return nullptr;
        TopoDS_Edge edge = edgeMaker.Edge();
        BRepLib::BuildCurve3d(edge);

        BRepBuilderAPI_MakeWire wireMaker(edge);
        if (!wireMaker.IsDone()) return nullptr;
        TopoDS_Wire spine = wireMaker.Wire();

        // Sweep profile along spine using MakePipeShell (Frenet trihedron)
        TopoDS_Wire profile = BRepTools::OuterWire(face);
        BRepOffsetAPI_MakePipeShell pipeShell(spine);
        pipeShell.SetMode(Standard_True); // Frenet trihedron
        Standard_Boolean correction = align_to_spine ? Standard_True : Standard_False;
        pipeShell.Add(profile, Standard_True, correction);
        pipeShell.Build();
        if (!pipeShell.IsDone()) return nullptr;
        pipeShell.MakeSolid();

        return std::make_unique<TopoDS_Shape>(pipeShell.Shape());
    } catch (const Standard_Failure&) {
        return nullptr;
    }
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

bool write_step_stream(const TopoDS_Shape& shape, RustWriter& writer) {
    RustWriteStreambuf sbuf(writer);
    std::ostream os(&sbuf);
    STEPControl_Writer step_writer;
    if (step_writer.Transfer(shape, STEPControl_AsIs) != IFSelect_RetDone) {
        return false;
    }
    return step_writer.WriteStream(os) == IFSelect_RetDone;
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
#include <Quantity_Color.hxx>

namespace cadrum {

// Traverse every label in the XDE document and record face-level colors.
// Uses TDF_ChildIterator with allLevels=true for a flat, efficient walk.
static void collect_face_colors(
    const Handle(TDocStd_Document)& doc,
    const Handle(XCAFDoc_ColorTool)& colorTool,
    std::unordered_map<uint64_t, std::array<float, 3>>& colorMap)
{
    for (TDF_ChildIterator it(doc->Main(), Standard_True); it.More(); it.Next()) {
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
        cafreader.SetColorMode(Standard_True);

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
        TDF_LabelSequence roots;
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
        TDF_Label rootLabel = shapeTool->AddShape(shape, Standard_False);

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
        cafwriter.SetColorMode(Standard_True);
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
        ShapeUpgrade_UnifySameDomain unifier(shape, Standard_True, Standard_True, Standard_True);
        unifier.AllowInternalEdges(Standard_False);
        unifier.Build();

        auto r = std::make_unique<CleanShape>();
        r->shape = unifier.Shape();

        Handle(BRepTools_History) history = unifier.History();
        if (!history.IsNull()) {
            for (TopExp_Explorer ex(shape, TopAbs_FACE); ex.More(); ex.Next()) {
                const TopoDS_Shape& old_face = ex.Current();
                uint64_t old_id = reinterpret_cast<uint64_t>(old_face.TShape().get());
                if (history->IsRemoved(old_face)) continue;
                const TopTools_ListOfShape& mods = history->Modified(old_face);
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

#include "chijin/src/ffi.rs.h"

// Implementation-only OCCT headers (not exposed via wrapper.h)
#include <Standard_Failure.hxx>
#include <TopoDS_Solid.hxx>
#include <TopoDS_Compound.hxx>
#include <TopAbs_ShapeEnum.hxx>
#include <TopoDS.hxx>

#include <BRepBuilderAPI_Copy.hxx>
#include <BRepBuilderAPI_MakeFace.hxx>
#include <BRepBuilderAPI_Transform.hxx>
#include <BRepPrimAPI_MakeBox.hxx>
#include <BRepPrimAPI_MakeCylinder.hxx>
#include <BRepPrimAPI_MakeHalfSpace.hxx>
#include <BRepPrimAPI_MakePrism.hxx>

#include <BRepAlgoAPI_BooleanOperation.hxx>
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

namespace chijin {

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

    // Reference point is on the OPPOSITE side of the normal.
    // This means the solid fills the half-space WHERE the normal points.
    double len = std::sqrt(nx*nx + ny*ny + nz*nz);
    gp_Pnt ref_point(ox - nx/len, oy - ny/len, oz - nz/len);

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

// Helper: collect cross-section faces produced at the tool boundary.
// Calls Modified() on every face of the tool and collects results.
static TopoDS_Shape collect_generated_faces(
    BRepAlgoAPI_BooleanOperation& op, const TopoDS_Shape& tool)
{
    BRep_Builder builder;
    TopoDS_Compound raw;
    builder.MakeCompound(raw);
    for (TopExp_Explorer ex(tool, TopAbs_FACE); ex.More(); ex.Next()) {
        for (const TopoDS_Shape& s : op.Modified(ex.Current())) {
            builder.Add(raw, s);
        }
    }

    // Deep-copy each face individually so it is independent of the operator.
    BRep_Builder builder2;
    TopoDS_Compound result;
    builder2.MakeCompound(result);
    for (TopExp_Explorer ex(raw, TopAbs_FACE); ex.More(); ex.Next()) {
        BRepBuilderAPI_Copy fc(ex.Current(), Standard_True, Standard_False);
        builder2.Add(result, fc.Shape());
    }
    return result;
}

std::unique_ptr<BooleanShape> boolean_fuse(
    const TopoDS_Shape& a, const TopoDS_Shape& b)
{
    try {
        BRepAlgoAPI_Fuse fuse(a, b);
        fuse.Build();
        if (!fuse.IsDone()) return nullptr;
        // union has no tool boundary — new_faces is empty
        BRep_Builder builder;
        TopoDS_Compound empty;
        builder.MakeCompound(empty);
        BRepBuilderAPI_Copy copier(fuse.Shape(), Standard_True, Standard_False);
        auto r = std::make_unique<BooleanShape>();
        r->shape = copier.Shape();
        r->new_faces = empty;
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
        TopoDS_Shape new_faces = collect_generated_faces(cut, b);
        BRepBuilderAPI_Copy copier(cut.Shape(), Standard_True, Standard_False);
        auto r = std::make_unique<BooleanShape>();
        r->shape = copier.Shape();
        r->new_faces = new_faces;
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
        TopoDS_Shape new_faces = collect_generated_faces(common, b);
        BRepBuilderAPI_Copy copier(common.Shape(), Standard_True, Standard_False);
        auto r = std::make_unique<BooleanShape>();
        r->shape = copier.Shape();
        r->new_faces = new_faces;
        return r;
    } catch (const Standard_Failure&) {
        return nullptr;
    }
}

std::unique_ptr<TopoDS_Shape> boolean_shape_shape(const BooleanShape& r) {
    return std::make_unique<TopoDS_Shape>(r.shape);
}

std::unique_ptr<TopoDS_Shape> boolean_shape_new_faces(const BooleanShape& r) {
    return std::make_unique<TopoDS_Shape>(r.new_faces);
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
    // Bug 5 fix: Use BRepBuilderAPI_Transform which creates a fully
    // transformed copy, properly propagating to all sub-shapes.
    gp_Trsf transform;
    transform.SetTranslation(gp_Vec(tx, ty, tz));

    BRepBuilderAPI_Transform transformer(shape, transform, Standard_True);
    return std::make_unique<TopoDS_Shape>(transformer.Shape());
}

bool shape_is_null(const TopoDS_Shape& shape) {
    return shape.IsNull();
}

uint32_t shape_shell_count(const TopoDS_Shape& shape) {
    uint32_t count = 0;
    for (TopExp_Explorer ex(shape, TopAbs_SHELL); ex.More(); ex.Next()) {
        ++count;
    }
    return count;
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

std::unique_ptr<TopoDS_Shape> face_to_shape(const TopoDS_Face& face) {
    return std::make_unique<TopoDS_Shape>(face);
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

} // namespace chijin

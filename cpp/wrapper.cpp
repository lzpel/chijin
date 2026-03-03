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

#ifdef CHIJIN_COLOR
// XDE (Application Framework) headers — only present when ApplicationFramework
// module is built (BUILD_MODULE_ApplicationFramework = ON in CMake).
// Note: XCAFApp_Application is intentionally NOT included to avoid pulling in
// the visualization stack (XCAFPrs_Driver → TKVCAF → TKV3d).
// TDocStd_Document can be constructed directly — no application is needed for
// our simple write/read use case (no undo, no native-format persistence).
#include <TDocStd_Document.hxx>
#include <XCAFDoc_DocumentTool.hxx>
#include <XCAFDoc_ColorTool.hxx>
#include <XCAFDoc_ShapeTool.hxx>
#include <STEPCAFControl_Reader.hxx>
#include <STEPCAFControl_Writer.hxx>
#include <Quantity_Color.hxx>
#include <TDF_LabelSequence.hxx>
#endif // CHIJIN_COLOR

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

// ==================== ColorMap ====================
#ifdef CHIJIN_COLOR

void ColorMap::set(const TopoDS_Shape& face, uint8_t r, uint8_t g, uint8_t b) {
    RGB rgb = {r, g, b};
    if (map_.IsBound(face)) {
        map_.ChangeFind(face) = rgb;
    } else {
        map_.Bind(face, rgb);
    }
}

bool ColorMap::get(const TopoDS_Shape& face, uint8_t& r, uint8_t& g, uint8_t& b) const {
    if (!map_.IsBound(face)) return false;
    const RGB& rgb = map_.Find(face);
    r = rgb[0]; g = rgb[1]; b = rgb[2];
    return true;
}

int32_t ColorMap::size() const {
    return static_cast<int32_t>(map_.Size());
}

// Free functions for cxx bridge
std::unique_ptr<ColorMap> colormap_new() {
    return std::make_unique<ColorMap>();
}

void colormap_set(ColorMap& map, const TopoDS_Face& face, uint8_t r, uint8_t g, uint8_t b) {
    map.set(face, r, g, b);
}

bool colormap_get(const ColorMap& map, const TopoDS_Face& face, uint8_t& r, uint8_t& g, uint8_t& b) {
    return map.get(face, r, g, b);
}

int32_t colormap_size(const ColorMap& map) {
    return map.size();
}

// ==================== Color Relay Helpers ====================

// Relay colors through a boolean operation:
// For each face in input_shape that has a color in old_map,
// find Modified/unmodified faces in the result and assign the same color.
static void relay_colors_from_input(
    BRepAlgoAPI_BooleanOperation& op,
    const TopoDS_Shape& input_shape,
    const ColorMap& old_map,
    ColorMap& out_map)
{
    for (TopExp_Explorer ex(input_shape, TopAbs_FACE); ex.More(); ex.Next()) {
        const TopoDS_Shape& old_face = ex.Current();
        uint8_t r, g, b;
        if (!old_map.get(old_face, r, g, b)) continue;

        const TopTools_ListOfShape& modified = op.Modified(old_face);
        if (!modified.IsEmpty()) {
            // Face was modified: assign color to all resulting faces
            for (const TopoDS_Shape& new_face : modified) {
                out_map.set(new_face, r, g, b);
            }
        } else if (!op.IsDeleted(old_face)) {
            // Face was not modified and not deleted: it survived as-is
            out_map.set(old_face, r, g, b);
        }
    }
}

// Remap colors from before_copy to after_copy using face enumeration order.
// Both shapes must have the same number of faces in the same order.
std::unique_ptr<ColorMap> remap_colors_after_copy(
    const TopoDS_Shape& before_copy,
    const TopoDS_Shape& after_copy,
    const ColorMap& src)
{
    auto result = std::make_unique<ColorMap>();

    // Build indexed maps for both shapes
    TopTools_IndexedMapOfShape before_faces, after_faces;
    TopExp::MapShapes(before_copy, TopAbs_FACE, before_faces);
    TopExp::MapShapes(after_copy, TopAbs_FACE, after_faces);

    int n = std::min(before_faces.Extent(), after_faces.Extent());
    for (int i = 1; i <= n; ++i) {
        uint8_t r, g, b;
        if (src.get(before_faces(i), r, g, b)) {
            result->set(after_faces(i), r, g, b);
        }
    }

    return result;
}

// ==================== Color-Relay Boolean Operations ====================

// Generic helper for colored boolean operations.
// Template parameter Op is BRepAlgoAPI_Fuse, BRepAlgoAPI_Cut, or BRepAlgoAPI_Common.
template<typename Op>
static std::unique_ptr<BooleanShapeColored> boolean_colored_impl(
    const TopoDS_Shape& a, const ColorMap& a_colors,
    const TopoDS_Shape& b, const ColorMap& b_colors,
    bool collect_new_faces_from_tool)
{
    try {
        Op op(a, b);
        op.Build();
        if (!op.IsDone()) return nullptr;

        // 1. Relay colors from both inputs BEFORE deep_copy
        ColorMap pre_copy_colors;
        relay_colors_from_input(op, a, a_colors, pre_copy_colors);
        relay_colors_from_input(op, b, b_colors, pre_copy_colors);

        // 2. Collect new faces (cut cross-sections) if applicable
        ColorMap pre_copy_new_colors;
        TopoDS_Shape new_faces_raw;
        if (collect_new_faces_from_tool) {
            new_faces_raw = collect_generated_faces(op, b);
            // New faces get no color by default (user can assign later)
        } else {
            BRep_Builder builder;
            TopoDS_Compound empty;
            builder.MakeCompound(empty);
            new_faces_raw = empty;
        }

        // 3. Deep-copy the result shape
        BRepBuilderAPI_Copy copier(op.Shape(), Standard_True, Standard_False);
        TopoDS_Shape result_shape = copier.Shape();

        // 4. Remap colors from pre-copy to post-copy
        auto result_colors = remap_colors_after_copy(
            op.Shape(), result_shape, pre_copy_colors);

        auto r = std::make_unique<BooleanShapeColored>();
        r->shape = result_shape;
        r->new_faces = new_faces_raw;
        r->shape_colors = std::move(*result_colors);
        // new_faces are already deep-copied individually in collect_generated_faces
        r->new_faces_colors = pre_copy_new_colors;
        return r;
    } catch (const Standard_Failure&) {
        return nullptr;
    }
}

std::unique_ptr<BooleanShapeColored> boolean_fuse_colored(
    const TopoDS_Shape& a, const ColorMap& a_colors,
    const TopoDS_Shape& b, const ColorMap& b_colors)
{
    return boolean_colored_impl<BRepAlgoAPI_Fuse>(
        a, a_colors, b, b_colors, false);
}

std::unique_ptr<BooleanShapeColored> boolean_cut_colored(
    const TopoDS_Shape& a, const ColorMap& a_colors,
    const TopoDS_Shape& b, const ColorMap& b_colors)
{
    return boolean_colored_impl<BRepAlgoAPI_Cut>(
        a, a_colors, b, b_colors, true);
}

std::unique_ptr<BooleanShapeColored> boolean_common_colored(
    const TopoDS_Shape& a, const ColorMap& a_colors,
    const TopoDS_Shape& b, const ColorMap& b_colors)
{
    return boolean_colored_impl<BRepAlgoAPI_Common>(
        a, a_colors, b, b_colors, true);
}

// Accessors for BooleanShapeColored
std::unique_ptr<TopoDS_Shape> colored_result_shape(const BooleanShapeColored& r) {
    return std::make_unique<TopoDS_Shape>(r.shape);
}

std::unique_ptr<TopoDS_Shape> colored_result_new_faces(const BooleanShapeColored& r) {
    return std::make_unique<TopoDS_Shape>(r.new_faces);
}

std::unique_ptr<ColorMap> colored_result_shape_colors(const BooleanShapeColored& r) {
    auto m = std::make_unique<ColorMap>();
    for (NCollection_DataMap<TopoDS_Shape, RGB, TopTools_ShapeMapHasher>::Iterator it(r.shape_colors.raw()); it.More(); it.Next()) {
        m->set(it.Key(), it.Value()[0], it.Value()[1], it.Value()[2]);
    }
    return m;
}

std::unique_ptr<ColorMap> colored_result_new_faces_colors(const BooleanShapeColored& r) {
    auto m = std::make_unique<ColorMap>();
    for (NCollection_DataMap<TopoDS_Shape, RGB, TopTools_ShapeMapHasher>::Iterator it(r.new_faces_colors.raw()); it.More(); it.Next()) {
        m->set(it.Key(), it.Value()[0], it.Value()[1], it.Value()[2]);
    }
    return m;
}

// ==================== Color-Relay Clean ====================

std::unique_ptr<TopoDS_Shape> clean_shape_colored(
    const TopoDS_Shape& shape, const ColorMap& in_colors, ColorMap& out_colors)
{
    try {
        ShapeUpgrade_UnifySameDomain unifier(shape, Standard_True, Standard_True, Standard_True);
        unifier.AllowInternalEdges(Standard_False);
        unifier.Build();

        TopoDS_Shape result = unifier.Shape();

        const Handle(BRepTools_History)& history = unifier.History();

        // Relay colors: for each original face, find its Modified result
        for (TopExp_Explorer ex(shape, TopAbs_FACE); ex.More(); ex.Next()) {
            const TopoDS_Shape& old_face = ex.Current();
            uint8_t r, g, b;
            if (!in_colors.get(old_face, r, g, b)) continue;

            const TopTools_ListOfShape& modified = history->Generated(old_face);
            if (!modified.IsEmpty()) {
                for (const TopoDS_Shape& new_face : modified) {
                    // Use first-found color: only set if not already colored
                    uint8_t dummy_r, dummy_g, dummy_b;
                    if (!out_colors.get(new_face, dummy_r, dummy_g, dummy_b)) {
                        out_colors.set(new_face, r, g, b);
                    }
                }
            }

            const TopTools_ListOfShape& mod_list = history->Modified(old_face);
            if (!mod_list.IsEmpty()) {
                for (const TopoDS_Shape& new_face : mod_list) {
                    uint8_t dummy_r, dummy_g, dummy_b;
                    if (!out_colors.get(new_face, dummy_r, dummy_g, dummy_b)) {
                        out_colors.set(new_face, r, g, b);
                    }
                }
            }

            // If neither Generated nor Modified, and face is not removed, it survives
            if (modified.IsEmpty() && mod_list.IsEmpty() && !history->IsRemoved(old_face)) {
                out_colors.set(old_face, r, g, b);
            }
        }

        return std::make_unique<TopoDS_Shape>(result);
    } catch (const Standard_Failure&) {
        return nullptr;
    }
}

// ==================== XDE STEP Colored I/O ====================

// Recursively extract face colors from an XDE document label tree.
// Stores each colored shape's color into out_colors using its TopoDS_Shape
// as key (IsSame()-based lookup).
static void extract_colors_from_label(
    const TDF_Label& label,
    const Handle(XCAFDoc_ShapeTool)& shape_tool,
    const Handle(XCAFDoc_ColorTool)& color_tool,
    ColorMap& out_colors)
{
    // Check this label for a color
    TopoDS_Shape shape = shape_tool->GetShape(label);
    if (!shape.IsNull()) {
        Quantity_Color color;
        bool found = color_tool->GetColor(label, XCAFDoc_ColorSurf, color);
        if (!found) found = color_tool->GetColor(label, XCAFDoc_ColorGen, color);
        if (found) {
            Standard_Real r, g, b;
            color.Values(r, g, b, Quantity_TOC_sRGB);
            out_colors.set(shape,
                static_cast<uint8_t>(std::lround(r * 255.0)),
                static_cast<uint8_t>(std::lround(g * 255.0)),
                static_cast<uint8_t>(std::lround(b * 255.0)));
        }
    }

    // Recurse into direct sub-shapes (faces, edges, ...)
    TDF_LabelSequence subshapes;
    shape_tool->GetSubShapes(label, subshapes);
    for (int i = 1; i <= subshapes.Length(); ++i) {
        extract_colors_from_label(subshapes.Value(i), shape_tool, color_tool, out_colors);
    }
}

std::unique_ptr<TopoDS_Shape> read_step_colored_from_slice(
    rust::Slice<const uint8_t> data, ColorMap& out_colors)
{
    // Copy slice into a string so we can use istringstream (seekable).
    std::string buf(reinterpret_cast<const char*>(data.data()), data.size());
    std::istringstream iss(std::move(buf));

    // Create the XDE document directly — no XCAFApp_Application needed.
    // TDocStd_Document creates its own TDF_Data (label tree) in the constructor.
    // Using the application singleton would pull in XCAFPrs_Driver → TKVCAF →
    // TKV3d, which we do not need.
    Handle(TDocStd_Document) doc = new TDocStd_Document("XmlXCAF");

    // Heap-allocate and intentionally leak the reader to prevent crash on
    // process exit (same pattern as read_step_stream — Bug 2 fix).
    auto* caf_reader = new STEPCAFControl_Reader();
    caf_reader->SetColorMode(Standard_True);

    if (caf_reader->ChangeReader().ReadStream("step", iss) != IFSelect_RetDone) {
        return nullptr;
    }

    if (!caf_reader->Transfer(doc, Message_ProgressRange())) {
        return nullptr;
    }

    Handle(XCAFDoc_ShapeTool) shape_tool =
        XCAFDoc_DocumentTool::ShapeTool(doc->Main());
    Handle(XCAFDoc_ColorTool) color_tool =
        XCAFDoc_DocumentTool::ColorTool(doc->Main());

    TDF_LabelSequence free_shapes;
    shape_tool->GetFreeShapes(free_shapes);
    if (free_shapes.IsEmpty()) {
        return nullptr;
    }

    // Build result: single shape or compound of all free shapes
    TopoDS_Shape result;
    if (free_shapes.Length() == 1) {
        result = shape_tool->GetShape(free_shapes.Value(1));
    } else {
        BRep_Builder builder;
        TopoDS_Compound compound;
        builder.MakeCompound(compound);
        for (int i = 1; i <= free_shapes.Length(); ++i) {
            builder.Add(compound, shape_tool->GetShape(free_shapes.Value(i)));
        }
        result = compound;
    }

    // Extract colors from each free shape's label tree
    for (int i = 1; i <= free_shapes.Length(); ++i) {
        extract_colors_from_label(
            free_shapes.Value(i), shape_tool, color_tool, out_colors);
    }

    // doc Handle goes out of scope here; TDocStd_Document is destroyed.
    // The extracted TopoDS_Shape objects hold independent Handle<TShape>
    // references and remain valid after the document is gone.
    return std::make_unique<TopoDS_Shape>(result);
}

rust::Vec<uint8_t> write_step_colored_to_vec(
    const TopoDS_Shape& shape, const ColorMap& colors)
{
    // Create XDE document directly (see read_step_colored_from_slice comment).
    Handle(TDocStd_Document) doc = new TDocStd_Document("XmlXCAF");

    Handle(XCAFDoc_ShapeTool) shape_tool =
        XCAFDoc_DocumentTool::ShapeTool(doc->Main());
    Handle(XCAFDoc_ColorTool) color_tool =
        XCAFDoc_DocumentTool::ColorTool(doc->Main());

    // Register the main shape in the document
    TDF_Label shape_label = shape_tool->NewShape();
    shape_tool->SetShape(shape_label, shape);

    // For each face with a color: add a sub-shape label and assign the color
    for (TopExp_Explorer ex(shape, TopAbs_FACE); ex.More(); ex.Next()) {
        const TopoDS_Shape& face = ex.Current();
        uint8_t r, g, b;
        if (!colors.get(face, r, g, b)) continue;

        TDF_Label face_label = shape_tool->AddSubShape(shape_label, face);
        Quantity_Color qcolor(
            static_cast<double>(r) / 255.0,
            static_cast<double>(g) / 255.0,
            static_cast<double>(b) / 255.0,
            Quantity_TOC_sRGB);
        color_tool->SetColor(face_label, qcolor, XCAFDoc_ColorSurf);
    }

    // Transfer the document to STEP model
    STEPCAFControl_Writer caf_writer;
    caf_writer.SetColorMode(Standard_True);
    if (!caf_writer.Transfer(doc)) {
        return rust::Vec<uint8_t>(); // empty = failure
    }

    // Write STEP model to an in-memory string buffer
    std::ostringstream oss;
    if (caf_writer.ChangeWriter().WriteStream(oss) != IFSelect_RetDone) {
        return rust::Vec<uint8_t>(); // empty = failure
    }

    std::string s = oss.str();
    rust::Vec<uint8_t> result;
    result.reserve(s.size());
    for (unsigned char c : s) {
        result.push_back(c);
    }
    return result;
}

#endif // CHIJIN_COLOR

} // namespace chijin


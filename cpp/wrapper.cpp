#include "wrapper.h"

#include <cmath>
#include <cstring>
#include <algorithm>

namespace chijin {

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
    RustReadStreambuf sbuf(reader);
    std::istream is(&sbuf);

    auto shape = std::make_unique<TopoDS_Shape>();
    BinTools::Read(*shape, is);

    if (shape->IsNull()) {
        return nullptr;
    }
    return shape;
}

bool write_brep_bin_stream(const TopoDS_Shape& shape, RustWriter& writer) {
    RustWriteStreambuf sbuf(writer);
    std::ostream os(&sbuf);
    BinTools::Write(shape, os);
    return os.good();
}

std::unique_ptr<TopoDS_Shape> read_brep_text_stream(RustReader& reader) {
    RustReadStreambuf sbuf(reader);
    std::istream is(&sbuf);

    auto shape = std::make_unique<TopoDS_Shape>();
    BRep_Builder builder;
    BRepTools::Read(*shape, is, builder);

    if (shape->IsNull()) {
        return nullptr;
    }
    return shape;
}

bool write_brep_text_stream(const TopoDS_Shape& shape, RustWriter& writer) {
    RustWriteStreambuf sbuf(writer);
    std::ostream os(&sbuf);
    BRepTools::Write(shape, os);
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

std::unique_ptr<TopoDS_Shape> boolean_fuse(
    const TopoDS_Shape& a, const TopoDS_Shape& b)
{
    BRepAlgoAPI_Fuse fuse(a, b);
    fuse.Build();
    if (!fuse.IsDone()) {
        return make_empty();
    }
    // Deep copy to sever shared Handle references (Bug 1 fix)
    BRepBuilderAPI_Copy copier(fuse.Shape(), Standard_True, Standard_False);
    return std::make_unique<TopoDS_Shape>(copier.Shape());
}

std::unique_ptr<TopoDS_Shape> boolean_cut(
    const TopoDS_Shape& a, const TopoDS_Shape& b)
{
    BRepAlgoAPI_Cut cut(a, b);
    cut.Build();
    if (!cut.IsDone()) {
        return make_empty();
    }
    BRepBuilderAPI_Copy copier(cut.Shape(), Standard_True, Standard_False);
    return std::make_unique<TopoDS_Shape>(copier.Shape());
}

std::unique_ptr<TopoDS_Shape> boolean_common(
    const TopoDS_Shape& a, const TopoDS_Shape& b)
{
    BRepAlgoAPI_Common common(a, b);
    common.Build();
    if (!common.IsDone()) {
        return make_empty();
    }
    BRepBuilderAPI_Copy copier(common.Shape(), Standard_True, Standard_False);
    return std::make_unique<TopoDS_Shape>(copier.Shape());
}

// ==================== Shape Methods ====================

std::unique_ptr<TopoDS_Shape> clean_shape(const TopoDS_Shape& shape) {
    ShapeUpgrade_UnifySameDomain unifier(shape, Standard_True, Standard_True, Standard_True);
    unifier.AllowInternalEdges(Standard_False);
    unifier.Build();
    return std::make_unique<TopoDS_Shape>(unifier.Shape());
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
    return std::make_unique<TopExp_Explorer>(shape, TopAbs_EDGE);
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
    GProp_GProps props;
    BRepGProp::SurfaceProperties(face, props);
    gp_Pnt center = props.CentreOfMass();
    cx = center.X();
    cy = center.Y();
    cz = center.Z();
}

void face_normal_at_center(const TopoDS_Face& face,
    double& nx, double& ny, double& nz)
{
    // Step 1: Get center of mass
    GProp_GProps props;
    BRepGProp::SurfaceProperties(face, props);
    gp_Pnt center = props.CentreOfMass();

    // Step 2: Get surface and project center point onto it
    Handle(Geom_Surface) surface = BRep_Tool::Surface(face);
    GeomAPI_ProjectPointOnSurf projector(center, surface);

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
}

std::unique_ptr<TopoDS_Shape> face_extrude(const TopoDS_Face& face,
    double dx, double dy, double dz)
{
    gp_Vec prism_vec(dx, dy, dz);
    BRepPrimAPI_MakePrism maker(face, prism_vec, Standard_False, Standard_True);
    return std::make_unique<TopoDS_Shape>(maker.Shape());
}

std::unique_ptr<TopoDS_Shape> face_to_shape(const TopoDS_Face& face) {
    return std::make_unique<TopoDS_Shape>(face);
}

// ==================== Edge Methods ====================

ApproxPoints edge_approximation_segments(
    const TopoDS_Edge& edge, double tolerance)
{
    ApproxPoints result;
    result.count = 0;

    BRepAdaptor_Curve curve(edge);
    // Bug 4 fix: tolerance is now a parameter instead of hardcoded 0.1
    GCPnts_TangentialDeflection approx(curve, tolerance, tolerance);

    int nb_points = approx.NbPoints();
    result.count = static_cast<uint32_t>(nb_points);

    for (int i = 1; i <= nb_points; i++) {
        gp_Pnt p = approx.Value(i);
        result.coords.push_back(p.X());
        result.coords.push_back(p.Y());
        result.coords.push_back(p.Z());
    }

    return result;
}

} // namespace chijin

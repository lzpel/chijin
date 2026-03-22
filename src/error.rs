/// Errors that can occur during OpenCASCADE operations.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// STEP file read failed (invalid format or corrupted data).
    #[error("STEP read failed")]
    StepReadFailed,

    /// BRep file read failed (invalid format or corrupted data).
    #[error("BRep read failed")]
    BrepReadFailed,

    /// STEP file write failed.
    #[error("STEP write failed")]
    StepWriteFailed,

    /// BRep file write failed.
    #[error("BRep write failed")]
    BrepWriteFailed,

    /// Triangulation/meshing failed.
    #[error("Triangulation failed")]
    TriangulationFailed,

    /// Boolean operation (fuse/cut/common) failed in OCCT.
    #[error("Boolean operation failed")]
    BooleanOperationFailed,

    /// Shape cleaning (UnifySameDomain) failed in OCCT.
    #[error("Shape clean failed")]
    CleanFailed,

    /// Face extrusion (MakePrism) failed in OCCT.
    #[error("Extrude failed")]
    ExtrudeFailed,

    /// Face revolution (MakeRevol) failed in OCCT.
    #[error("Revolve failed")]
    RevolveFailed,

    /// Helix sweep (MakePipeShell) failed in OCCT.
    #[error("Helix failed")]
    HelixFailed,

    /// Face creation from polygon points failed (non-planar or degenerate points).
    #[error("Invalid polygon")]
    InvalidPolygon,

    /// SVG export (HLR projection) failed.
    #[error("SVG export failed")]
    SvgExportFailed,
}

/// Errors that can occur during CAD operations.
#[derive(Debug)]
pub enum Error {
    /// STEP file read failed (invalid format or corrupted data).
    StepReadFailed,

    /// BRep file read failed (invalid format or corrupted data).
    BrepReadFailed,

    /// STEP file write failed.
    StepWriteFailed,

    /// BRep file write failed.
    BrepWriteFailed,

    /// Triangulation/meshing failed.
    TriangulationFailed,

    /// Boolean operation (fuse/cut/common) failed.
    BooleanOperationFailed,

    /// Shape cleaning (UnifySameDomain) failed.
    CleanFailed,

    /// Face extrusion (MakePrism) failed.
    ExtrudeFailed,

    /// Face revolution (MakeRevol) failed.
    RevolveFailed,

    /// Helix sweep (MakePipeShell) failed.
    HelixFailed,

    /// Face creation from polygon points failed (non-planar or degenerate points).
    InvalidPolygon,

    /// SVG export (HLR projection) failed.
    SvgExportFailed,

    /// Invalid color string (unrecognized name or invalid hex format).
    InvalidColor(String),

    /// Unknown error.
    Unknown(String),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::StepReadFailed => write!(f, "STEP read failed"),
            Error::BrepReadFailed => write!(f, "BRep read failed"),
            Error::StepWriteFailed => write!(f, "STEP write failed"),
            Error::BrepWriteFailed => write!(f, "BRep write failed"),
            Error::TriangulationFailed => write!(f, "Triangulation failed"),
            Error::BooleanOperationFailed => write!(f, "Boolean operation failed"),
            Error::CleanFailed => write!(f, "Shape clean failed"),
            Error::ExtrudeFailed => write!(f, "Extrude failed"),
            Error::RevolveFailed => write!(f, "Revolve failed"),
            Error::HelixFailed => write!(f, "Helix failed"),
            Error::InvalidPolygon => write!(f, "Invalid polygon"),
            Error::SvgExportFailed => write!(f, "SVG export failed"),
            Error::InvalidColor(s) => write!(f, "Invalid color: \"{}\"", s),
            Error::Unknown(msg) => write!(f, "Unknown error: {}", msg),
        }
    }
}

impl std::error::Error for Error {}

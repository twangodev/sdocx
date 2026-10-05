/// Why shape or line geometry was not emitted during an admitted render attempt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub enum GeometryDiagnosticKind {
    InvalidPath,
    UnsupportedPath,
    UnrepresentablePath,
    InvalidGeometry,
    UnsupportedShapeTemplate,
    UnsupportedLineType,
    MissingLinePath,
}

/// A rejected shape or line, identified within its rendered backing page.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GeometryDiagnostic {
    /// Payload offset in the original page bytes; absent for constructed objects.
    pub source_offset: Option<usize>,
    pub object_uuid: String,
    pub kind: GeometryDiagnosticKind,
}

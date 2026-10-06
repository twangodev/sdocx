/// The affected paint component of a shape or line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub enum PaintRole {
    Fill,
    Outline,
}

/// Why a shape or line paint component was omitted during rendering.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub enum PaintDiagnosticKind {
    /// The saved paint type has no admitted rendering implementation.
    UnsupportedPaint,
    /// An enabled paint has no retained source.
    MissingPaintSource,
    /// The saved gradient stops or parameters do not meet admission constraints.
    InvalidGradient,
    /// The saved shape frame is outside the supported native gradient mapping.
    UnsupportedGradientFrame,
    /// Admitted gradient data cannot produce finite, representable output geometry.
    UnrepresentableGradient,
}

/// An omitted fill or outline, identified within its rendered backing page.
/// Geometry and independently admitted paint components remain eligible for rendering.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PaintDiagnostic {
    /// Payload offset in the original page bytes; absent for constructed objects.
    pub source_offset: Option<usize>,
    pub object_uuid: String,
    pub role: PaintRole,
    pub kind: PaintDiagnosticKind,
}

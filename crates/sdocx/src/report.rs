/// Non-fatal findings produced while interpreting an `.sdocx` archive.
#[derive(Debug, Clone, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ParseReport {
    /// Findings in deterministic archive/page order.
    pub diagnostics: Vec<ParseDiagnostic>,
}

impl ParseReport {
    /// Whether the parser found anything callers should surface or inspect.
    pub fn has_warnings(&self) -> bool {
        !self.diagnostics.is_empty()
    }

    pub(crate) fn warning(
        &mut self,
        code: DiagnosticCode,
        archive_entry: Option<String>,
        message: impl Into<String>,
    ) {
        self.diagnostics.push(ParseDiagnostic {
            severity: DiagnosticSeverity::Warning,
            code,
            archive_entry,
            message: message.into(),
        });
    }
}

/// One non-fatal parser finding.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ParseDiagnostic {
    /// Finding severity.
    pub severity: DiagnosticSeverity,
    /// Stable machine-readable category.
    pub code: DiagnosticCode,
    /// Related ZIP entry, when the finding belongs to one entry.
    pub archive_entry: Option<String>,
    /// Human-readable detail.
    pub message: String,
}

/// Severity of a parser diagnostic.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub enum DiagnosticSeverity {
    /// Content was retained or skipped safely, but deserves inspection.
    Warning,
}

/// Stable category for a parser diagnostic.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub enum DiagnosticCode {
    /// `pageIdInfo.dat` is absent, so filename order is used.
    MissingPageManifest,
    /// A manifest page identifier has no matching `.page` entry.
    MissingPageEntry,
    /// A `.page` entry is not listed in the page manifest.
    UnlistedPageEntry,
    /// The archive filename and embedded page UUID disagree.
    PageIdentifierMismatch,
    /// An object identifier is newer than the currently known SDK mapping.
    UnknownObjectType,
    /// A text box contains optional fields or records without full semantic support.
    UnsupportedTextBoxFeature,
}

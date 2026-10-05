use serde::{Serialize, Serializer};

#[derive(Serialize)]
pub(super) struct SvgOutput {
    pub svg: String,
    pub page_index: usize,
    pub source_page_index: usize,
    pub text_diagnostics: Vec<sdocx::TextDiagnostic>,
    pub object_diagnostics: Vec<sdocx::ObjectDiagnostic>,
    pub geometry_diagnostics: Vec<sdocx::GeometryDiagnostic>,
}

impl SvgOutput {
    pub fn new(page_index: usize, page: sdocx::RenderedPage) -> Self {
        Self {
            svg: page.svg,
            page_index,
            source_page_index: page.source_page_index,
            text_diagnostics: page.text_diagnostics,
            object_diagnostics: page.object_diagnostics,
            geometry_diagnostics: page.geometry_diagnostics,
        }
    }
}

#[derive(Serialize)]
pub(super) struct PdfOutput {
    #[serde(serialize_with = "serialize_bytes")]
    pub bytes: Vec<u8>,
    pub pages: Vec<sdocx::PdfPageDiagnostics>,
}

impl From<sdocx::PdfOutput> for PdfOutput {
    fn from(output: sdocx::PdfOutput) -> Self {
        Self {
            bytes: output.bytes,
            pages: output.pages,
        }
    }
}

fn serialize_bytes<S: Serializer>(bytes: &[u8], serializer: S) -> Result<S::Ok, S::Error> {
    serializer.serialize_bytes(bytes)
}

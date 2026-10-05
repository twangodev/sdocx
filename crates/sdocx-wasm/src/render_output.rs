use serde::{Serialize, Serializer};

#[derive(Serialize)]
pub(super) struct SvgOutput {
    pub svg: String,
    pub page_index: usize,
    pub text_diagnostics: Vec<sdocx::TextDiagnostic>,
    pub object_diagnostics: Vec<sdocx::ObjectDiagnostic>,
}

impl SvgOutput {
    pub fn new(page_index: usize, page: sdocx::RenderedPage) -> Self {
        Self {
            svg: page.svg,
            page_index,
            text_diagnostics: page.text_diagnostics,
            object_diagnostics: page.object_diagnostics,
        }
    }
}

#[derive(Serialize)]
pub(super) struct PdfOutput {
    #[serde(serialize_with = "serialize_bytes")]
    pub bytes: Vec<u8>,
    pub pages: Vec<PageDiagnostics>,
}

impl From<sdocx::PdfOutput> for PdfOutput {
    fn from(output: sdocx::PdfOutput) -> Self {
        Self {
            bytes: output.bytes,
            pages: output.pages.into_iter().map(Into::into).collect(),
        }
    }
}

#[derive(Serialize)]
pub(super) struct PageDiagnostics {
    pub page_index: usize,
    pub text_diagnostics: Vec<sdocx::TextDiagnostic>,
    pub object_diagnostics: Vec<sdocx::ObjectDiagnostic>,
}

impl From<sdocx::PdfPageDiagnostics> for PageDiagnostics {
    fn from(page: sdocx::PdfPageDiagnostics) -> Self {
        Self {
            page_index: page.page_index,
            text_diagnostics: page.text_diagnostics,
            object_diagnostics: page.object_diagnostics,
        }
    }
}

fn serialize_bytes<S: Serializer>(bytes: &[u8], serializer: S) -> Result<S::Ok, S::Error> {
    serializer.serialize_bytes(bytes)
}

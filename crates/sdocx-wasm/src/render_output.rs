use serde::{Serialize, Serializer};

#[derive(Serialize)]
pub(super) struct SvgOutput {
    pub svg: String,
    pub page_index: usize,
    pub source_page_index: usize,
    pub text_diagnostics: Vec<sdocx::TextDiagnostic>,
    pub object_diagnostics: Vec<sdocx::ObjectDiagnostic>,
    pub geometry_diagnostics: Vec<sdocx::GeometryDiagnostic>,
    pub paint_diagnostics: Vec<sdocx::PaintDiagnostic>,
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
            paint_diagnostics: page.paint_diagnostics,
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

#[cfg(test)]
mod tests {
    use super::{PdfOutput, SvgOutput};
    use sdocx::{PaintDiagnostic, PaintDiagnosticKind, PaintRole, RenderedPage};

    #[test]
    fn detailed_outputs_serialize_independent_paint_roles_and_identity() {
        let paint = vec![
            PaintDiagnostic {
                source_offset: Some(321),
                object_uuid: "shape paint".into(),
                role: PaintRole::Fill,
                kind: PaintDiagnosticKind::UnsupportedGradientFrame,
            },
            PaintDiagnostic {
                source_offset: None,
                object_uuid: "line paint".into(),
                role: PaintRole::Outline,
                kind: PaintDiagnosticKind::InvalidGradient,
            },
        ];
        let page = RenderedPage {
            source_page_index: 7,
            width: 64,
            height: 64,
            svg: "<svg><path stroke=\"#112233\"/></svg>".into(),
            text_diagnostics: Vec::new(),
            object_diagnostics: Vec::new(),
            geometry_diagnostics: Vec::new(),
            paint_diagnostics: paint.clone(),
        };
        let expected = serde_json::json!([
            { "source_offset": 321, "object_uuid": "shape paint", "role": "Fill", "kind": "UnsupportedGradientFrame" },
            { "source_offset": null, "object_uuid": "line paint", "role": "Outline", "kind": "InvalidGradient" }
        ]);
        let svg = serde_json::to_value(SvgOutput::new(2, page)).unwrap();
        assert_eq!(svg["paint_diagnostics"], expected);
        assert_eq!(svg["geometry_diagnostics"], serde_json::json!([]));
        assert!(svg["svg"].as_str().unwrap().contains("stroke="));
        assert_eq!(svg["page_index"], 2);
        assert_eq!(svg["source_page_index"], 7);
        let pdf = PdfOutput::from(sdocx::PdfOutput {
            bytes: b"%PDF".to_vec(),
            pages: [2, 0, 2]
                .into_iter()
                .map(|page_index| sdocx::PdfPageDiagnostics {
                    page_index,
                    source_page_index: 7,
                    text_diagnostics: Vec::new(),
                    object_diagnostics: Vec::new(),
                    geometry_diagnostics: Vec::new(),
                    paint_diagnostics: paint.clone(),
                })
                .collect(),
        });
        let pdf = serde_json::to_value(pdf).unwrap();
        for (report, page_index) in pdf["pages"].as_array().unwrap().iter().zip([2, 0, 2]) {
            assert_eq!(report["page_index"], page_index);
            assert_eq!(report["source_page_index"], 7);
            assert_eq!(report["paint_diagnostics"], expected);
            assert_eq!(report["geometry_diagnostics"], serde_json::json!([]));
        }
    }
}

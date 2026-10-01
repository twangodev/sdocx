use lopdf::{Object, content::Content};
use svgtypes::Transform;

#[derive(Default)]
pub struct PdfGeometry {
    pub text: Vec<(String, f64, f64)>,
    pub source: String,
    pub extracted_text: String,
    pub actual_text: Vec<String>,
    pub images: Vec<[f64; 4]>,
    pub image_resources: usize,
}

fn number(value: &Object) -> f64 {
    f64::from(value.as_float().unwrap())
}

fn matrix(values: &[Object]) -> Transform {
    Transform::new(
        number(&values[0]),
        number(&values[1]),
        number(&values[2]),
        number(&values[3]),
        number(&values[4]),
        number(&values[5]),
    )
}

fn compose(left: Transform, right: Transform) -> Transform {
    Transform::new(
        left.a * right.a + left.c * right.b,
        left.b * right.a + left.d * right.b,
        left.a * right.c + left.c * right.d,
        left.b * right.c + left.d * right.d,
        left.a * right.e + left.c * right.f + left.e,
        left.b * right.e + left.d * right.f + left.f,
    )
}

fn actual_text(value: &Object) -> String {
    lopdf::decode_text_string(value).unwrap()
}

pub fn read(bytes: &[u8], dpi: f64) -> PdfGeometry {
    let pdf = lopdf::Document::load_mem(bytes).unwrap();
    assert_eq!(pdf.get_pages().len(), 1);
    let page_id = pdf.get_pages()[&1];
    let page = pdf.get_dictionary(page_id).unwrap();
    let height = number(&page.get(b"MediaBox").unwrap().as_array().unwrap()[3]);
    let scale = dpi / 72.0;
    let fonts = pdf.get_page_fonts(page_id).unwrap();
    let resources = pdf
        .dereference(page.get(b"Resources").unwrap())
        .unwrap()
        .1
        .as_dict()
        .unwrap();
    let xobjects = resources
        .get(b"XObject")
        .ok()
        .map(|value| pdf.dereference(value).unwrap().1.as_dict().unwrap());
    let mut result = PdfGeometry {
        extracted_text: pdf.extract_text(&[1]).unwrap(),
        image_resources: pdf
            .objects
            .values()
            .filter(|object| {
                object.as_stream().is_ok_and(|stream| {
                    stream
                        .dict
                        .get(b"Subtype")
                        .is_ok_and(|kind| kind.as_name().is_ok_and(|name| name == b"Image"))
                })
            })
            .count(),
        ..Default::default()
    };
    let content = Content::decode(&pdf.get_page_content(page_id).unwrap()).unwrap();
    let mut ctm = Transform::default();
    let mut stack = Vec::new();
    let mut tm = Transform::default();
    let mut font = Vec::new();
    let mut marked_text = Vec::new();
    for operation in content.operations {
        match operation.operator.as_str() {
            "q" => stack.push(ctm),
            "Q" => ctm = stack.pop().unwrap(),
            "cm" => ctm = compose(ctm, matrix(&operation.operands)),
            "Tm" => tm = matrix(&operation.operands),
            "Tf" => font = operation.operands[0].as_name().unwrap().to_vec(),
            "BDC" => {
                let replacement = operation.operands[1]
                    .as_dict()
                    .unwrap()
                    .get(b"ActualText")
                    .ok()
                    .map(actual_text);
                if let Some(value) = &replacement {
                    result.actual_text.push(value.clone());
                }
                marked_text.push((replacement, false));
            }
            "BMC" => marked_text.push((None, false)),
            "EMC" => {
                marked_text.pop().unwrap();
            }
            "Tj" | "TJ" => {
                let encoding = fonts[&font].get_font_encoding(&pdf).unwrap();
                let values = if let Ok(values) = operation.operands[0].as_array() {
                    values.as_slice()
                } else {
                    operation.operands.as_slice()
                };
                for value in values {
                    if let Object::String(bytes, _) = value {
                        let decoded = lopdf::Document::decode_text(&encoding, bytes).unwrap();
                        if let Some((Some(replacement), emitted)) = marked_text
                            .iter_mut()
                            .find(|(replacement, _)| replacement.is_some())
                        {
                            if !*emitted {
                                result.source.push_str(replacement);
                                *emitted = true;
                            }
                        } else {
                            result.source.push_str(&decoded);
                        }
                        let transform = compose(ctm, tm);
                        result.text.push((
                            decoded,
                            transform.e * scale,
                            (height - transform.f) * scale,
                        ));
                    }
                }
            }
            "Do" => {
                let resource = xobjects
                    .unwrap()
                    .get(operation.operands[0].as_name().unwrap())
                    .unwrap();
                let image = pdf.dereference(resource).unwrap().1.as_stream().unwrap();
                assert_eq!(
                    image.dict.get(b"Subtype").unwrap().as_name().unwrap(),
                    b"Image"
                );
                let corners = [(0.0, 0.0), (1.0, 0.0), (0.0, 1.0), (1.0, 1.0)].map(|(x, y)| {
                    (
                        (ctm.a * x + ctm.c * y + ctm.e) * scale,
                        (height - (ctm.b * x + ctm.d * y + ctm.f)) * scale,
                    )
                });
                result.images.push([
                    corners
                        .iter()
                        .map(|point| point.0)
                        .fold(f64::INFINITY, f64::min),
                    corners
                        .iter()
                        .map(|point| point.1)
                        .fold(f64::INFINITY, f64::min),
                    corners
                        .iter()
                        .map(|point| point.0)
                        .fold(f64::NEG_INFINITY, f64::max),
                    corners
                        .iter()
                        .map(|point| point.1)
                        .fold(f64::NEG_INFINITY, f64::max),
                ]);
            }
            _ => {}
        }
    }
    result
}

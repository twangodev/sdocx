use std::collections::HashMap;

use lopdf::{Object, ObjectId, content::Content};
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

#[allow(dead_code)]
pub fn tagged_source(bytes: &[u8]) -> String {
    let pdf = lopdf::Document::load_mem(bytes).unwrap();
    let mut sources = HashMap::new();
    for page_id in pdf.get_pages().into_values() {
        let fonts = pdf.get_page_fonts(page_id).unwrap();
        let content = Content::decode(&pdf.get_page_content(page_id).unwrap()).unwrap();
        let mut marked: Vec<(Option<i64>, Option<String>, bool)> = Vec::new();
        let mut font = Vec::new();
        for operation in content.operations {
            match operation.operator.as_str() {
                "BDC" => {
                    let properties = pdf.dereference(&operation.operands[1]).unwrap().1;
                    let properties = properties.as_dict().unwrap();
                    let mcid = properties
                        .get(b"MCID")
                        .ok()
                        .map(|value| value.as_i64().unwrap());
                    let replacement = properties.get(b"ActualText").ok().map(actual_text);
                    if let Some(mcid) = mcid {
                        assert!(sources.insert((page_id, mcid), String::new()).is_none());
                    }
                    marked.push((mcid, replacement, false));
                }
                "BMC" => marked.push((None, None, false)),
                "EMC" => {
                    marked.pop().unwrap();
                }
                "Tf" => font = operation.operands[0].as_name().unwrap().to_vec(),
                "Tj" | "TJ" => {
                    let values = operation.operands[0]
                        .as_array()
                        .map_or(operation.operands.as_slice(), Vec::as_slice);
                    for value in values {
                        let Object::String(bytes, _) = value else {
                            continue;
                        };
                        let replacement = marked
                            .iter()
                            .position(|(_, replacement, _)| replacement.is_some());
                        let mcid = replacement
                            .and_then(|index| marked[index].0)
                            .or_else(|| marked.iter().rev().find_map(|(mcid, _, _)| *mcid));
                        let Some(mcid) = mcid else { continue };
                        let source = if let Some(index) = replacement {
                            if marked[index].2 {
                                continue;
                            }
                            marked[index].2 = true;
                            marked[index].1.clone().unwrap()
                        } else {
                            let encoding = fonts[&font].get_font_encoding(&pdf).unwrap();
                            lopdf::Document::decode_text(&encoding, bytes).unwrap()
                        };
                        sources.get_mut(&(page_id, mcid)).unwrap().push_str(&source);
                    }
                }
                _ => {}
            }
        }
        assert!(marked.is_empty());
    }

    fn visit(
        pdf: &lopdf::Document,
        object: &Object,
        page: Option<ObjectId>,
        sources: &HashMap<(ObjectId, i64), String>,
        output: &mut String,
        depth: usize,
    ) {
        assert!(
            depth < 64,
            "cyclic or excessively nested PDF structure tree"
        );
        let object = pdf.dereference(object).unwrap().1;
        match object {
            Object::Array(children) => {
                for child in children {
                    visit(pdf, child, page, sources, output, depth + 1);
                }
            }
            Object::Integer(mcid) => output.push_str(&sources[&(page.unwrap(), *mcid)]),
            Object::Dictionary(node) => {
                let page = node
                    .get(b"Pg")
                    .ok()
                    .map(|page| page.as_reference().unwrap())
                    .or(page);
                if let Ok(replacement) = node.get(b"ActualText") {
                    output.push_str(&actual_text(replacement));
                } else if let Ok(mcid) = node.get(b"MCID") {
                    output.push_str(&sources[&(page.unwrap(), mcid.as_i64().unwrap())]);
                } else if let Ok(children) = node.get(b"K") {
                    visit(pdf, children, page, sources, output, depth + 1);
                } else {
                    assert_eq!(node.get(b"Type").unwrap().as_name().unwrap(), b"OBJR");
                }
            }
            Object::Null => {}
            other => panic!("unexpected PDF structure child: {other:?}"),
        }
    }

    let catalog = pdf
        .dereference(pdf.trailer.get(b"Root").unwrap())
        .unwrap()
        .1
        .as_dict()
        .unwrap();
    let mut output = String::new();
    visit(
        &pdf,
        catalog.get(b"StructTreeRoot").unwrap(),
        None,
        &sources,
        &mut output,
        0,
    );
    output
}

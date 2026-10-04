use crate::source_summary::{self, MediaManifestSummary, PageManifestSummary};
use sdocx::{LayoutDocument, ObjectType, ParsedDocument, StoredObject};
use serde::Serialize;
use serde_json::{Value, json};
use std::collections::HashMap;
use std::io::{Cursor, Read};
use wasm_bindgen::JsError;

pub struct Source {
    bytes: Vec<u8>,
    zip_length: usize,
    stroke_resources: Option<sdocx::StrokeResources>,
    cache: Option<(usize, Vec<u8>)>,
}

fn value<T: Serialize>(v: T) -> Value {
    serde_json::to_value(v).unwrap_or_else(|e| json!({"error": e.to_string()}))
}
fn decoded<T: Serialize>(v: sdocx::Result<T>) -> Value {
    match v {
        Ok(v) => value(v),
        Err(e) => json!({"error":e.to_string()}),
    }
}
fn number(v: &Value, key: &str) -> Result<usize, String> {
    v[key]
        .as_u64()
        .and_then(|n| usize::try_from(n).ok())
        .ok_or_else(|| format!("invalid {key}"))
}
fn safe_numbers(v: &mut Value) {
    match v {
        Value::Number(n)
            if n.as_i64()
                .is_some_and(|n| n.unsigned_abs() > 9_007_199_254_740_991)
                || n.as_u64().is_some_and(|n| n > 9_007_199_254_740_991) =>
        {
            *v = Value::String(n.to_string())
        }
        Value::Array(a) => a.iter_mut().for_each(safe_numbers),
        Value::Object(m) => {
            for (k, v) in m {
                if (k.ends_with("_raw") && k.contains("time")) && v.is_number() {
                    *v = Value::String(v.to_string());
                } else {
                    safe_numbers(v);
                }
            }
        }
        _ => {}
    }
}
fn find(objects: &[StoredObject], offset: usize) -> Option<&StoredObject> {
    for o in objects {
        if o.payload_offset == offset {
            return Some(o);
        }
        if let Some(found) = find(&o.children, offset) {
            return Some(found);
        }
    }
    None
}
fn index_objects<'a>(objects: &'a [StoredObject], index: &mut HashMap<usize, &'a StoredObject>) {
    for object in objects {
        index.insert(object.payload_offset, object);
        index_objects(&object.children, index);
    }
}
fn nodes(objects: &[StoredObject]) -> Value {
    value(
        objects
            .iter()
            .map(|o| {
                json!({"offset":o.payload_offset,"size":o.payload_size,
        "type":o.object_type,"children":o.children.len()})
            })
            .collect::<Vec<_>>(),
    )
}

impl Source {
    pub fn new(bytes: &[u8]) -> Result<Self, JsError> {
        let zip_length =
            sdocx::archive_zip_length(bytes).map_err(|e| JsError::new(&e.to_string()))? as usize;
        Ok(Self {
            bytes: bytes.to_vec(),
            zip_length,
            stroke_resources: None,
            cache: None,
        })
    }
    fn archive(&self) -> Result<zip::ZipArchive<Cursor<&[u8]>>, String> {
        zip::ZipArchive::new(Cursor::new(&self.bytes[..self.zip_length])).map_err(|e| e.to_string())
    }
    fn entry_index(&self, name: &str) -> Result<usize, String> {
        let mut archive = self.archive()?;
        for i in 0..archive.len() {
            if archive.by_index(i).map_err(|e| e.to_string())?.name() == name {
                return Ok(i);
            }
        }
        Err("entry not found".into())
    }
    fn entry(&mut self, index: usize) -> Result<&[u8], String> {
        if self.cache.as_ref().map(|c| c.0) != Some(index) {
            // Release the previous expansion before allocating another entry.
            self.cache = None;
            let mut archive = self.archive()?;
            let entry = archive.by_index(index).map_err(|e| e.to_string())?;
            let limit = super::MAX_BROWSER_ENTRY_SIZE;
            if entry.size() > limit {
                return Err("entry exceeds browser limit".into());
            }
            let mut data = Vec::new();
            entry
                .take(limit + 1)
                .read_to_end(&mut data)
                .map_err(|e| e.to_string())?;
            if data.len() as u64 > limit {
                return Err("entry exceeds browser limit".into());
            }
            self.cache = Some((index, data));
        }
        Ok(&self.cache.as_ref().unwrap().1)
    }
    #[cfg(test)]
    pub fn request(
        &mut self,
        parsed: &ParsedDocument,
        layout: &LayoutDocument,
        request: &str,
    ) -> Result<String, String> {
        self.request_with_fonts(parsed, layout, request, &sdocx::fonts::FontBook::default())
    }

    #[cfg(test)]
    pub fn request_with_fonts(
        &mut self,
        parsed: &ParsedDocument,
        layout: &LayoutDocument,
        request: &str,
        fonts: &sdocx::fonts::FontBook,
    ) -> Result<String, String> {
        self.request_with_text_cache(
            parsed,
            layout,
            request,
            fonts,
            &mut sdocx::DocumentTextCache::default(),
        )
    }

    pub fn request_with_text_cache(
        &mut self,
        parsed: &ParsedDocument,
        layout: &LayoutDocument,
        request: &str,
        fonts: &sdocx::fonts::FontBook,
        text_cache: &mut sdocx::DocumentTextCache,
    ) -> Result<String, String> {
        let r: Value = serde_json::from_str(request).map_err(|e| e.to_string())?;
        let limits = super::browser_parse_options().limits;
        let resources = if r["kind"].as_str() == Some("object") {
            if self.stroke_resources.is_none() {
                let resources = if let Some(note) = &parsed.note {
                    self.entry_index("note.note")
                        .ok()
                        .and_then(|index| self.entry(index).ok())
                        .and_then(|bytes| note.metadata_with_limits(bytes, &limits).ok())
                        .and_then(|metadata| metadata.string_table)
                        .map(|table| sdocx::StrokeResources::new(&table))
                        .unwrap_or_default()
                } else {
                    sdocx::StrokeResources::default()
                };
                self.stroke_resources = Some(resources);
            }
            self.stroke_resources.clone().unwrap_or_default()
        } else {
            sdocx::StrokeResources::default()
        };
        let mut result = match r["kind"].as_str().unwrap_or("") {
            "index" => {
                let mut archive = self.archive()?;
                let mut entries = Vec::new();
                for i in 0..archive.len() {
                    let e = archive.by_index(i).map_err(|e| e.to_string())?;
                    entries.push(json!({"index":i,"name":e.name(),"size":e.size(),"compressedSize":e.compressed_size(),"crc32":e.crc32()}));
                }
                json!({"entries":entries,"sourceSize":self.bytes.len(),"zipLength":self.zip_length,
                    "pages":parsed.stored_pages.iter().enumerate().map(|(i,p)| json!({"index":i,"visibleIndex":layout.pages.iter().position(|p| p.source_page_index == i),"name":p.archive_entry,"id":p.page.header.uuid})).collect::<Vec<_>>()})
            }
            "document" => {
                let m = &parsed.document.metadata;
                let metadata = json!({"format_version":m.format_version,"created_ms":m.created_ms,"modified_ms":m.modified_ms,
                    "background_color":m.background_color,"dark_mode_compatibility":m.dark_mode_compatibility,
                    "default_page_dimensions":m.default_page_dimensions,"page_mode":m.page_mode,"orientation":m.orientation,
                    "page_dimensions":m.page_dimensions,"flow_dimensions":m.flow_dimensions,"flow_page_padding":m.flow_page_padding,
                    "page_ids":m.page_ids,"note_text":m.note_text,"note_title":m.note_title,
                    "media_manifest":m.media_manifest.as_ref().map(MediaManifestSummary::from),
                    "archive_resources":source_summary::archive_resources(m),
                    "media_assets":m.media_assets.iter().map(|a| json!({"name":a.name,"archive_id":a.archive_id,"mime_type":a.mime_type,"byte_length":a.data.len()})).collect::<Vec<_>>()});
                json!({"metadata":metadata,"note":parsed.note,"endTag":parsed.end_tag,"endTagSource":parsed.end_tag_source,
                "manifest":parsed.page_manifest.as_ref().map(PageManifestSummary::from),"integrity":parsed.integrity,"diagnostics":parsed.report})
            }
            "bytes" => {
                let offset = number(&r, "offset")?;
                let length = number(&r, "length")?.min(4096);
                let bytes = if r["entry"].is_null() {
                    self.bytes.as_slice()
                } else {
                    self.entry(number(&r, "entry")?)?
                };
                if offset > bytes.len() {
                    return Err("byte offset out of bounds".into());
                }
                let end = offset.saturating_add(length).min(bytes.len());
                json!({"offset":offset,"total":bytes.len(),"bytes":&bytes[offset..end]})
            }
            "entry" => {
                let index = number(&r, "entry")?;
                let name = self
                    .archive()?
                    .by_index(index)
                    .map_err(|e| e.to_string())?
                    .name()
                    .to_owned();
                let bytes = self.entry(index)?;
                match name.as_str() {
                    "note.note" => decoded(
                        sdocx::parse_note_bytes_with_limits(bytes, &limits)
                            .and_then(|n| n.metadata_with_limits(bytes, &limits)),
                    ),
                    "media/mediaInfo.dat" => decoded(
                        sdocx::parse_media_manifest_bytes_with_limits(bytes, &limits)
                            .map(|manifest| value(MediaManifestSummary::from(&manifest))),
                    ),
                    "pageIdInfo.dat" => decoded(
                        sdocx::parse_page_manifest_bytes_with_limits(bytes, &limits)
                            .map(|manifest| value(PageManifestSummary::from(&manifest))),
                    ),
                    "end_tag.bin" => {
                        decoded(sdocx::parse_end_tag_bytes_with_limits(bytes, &limits))
                    }
                    _ => {
                        json!({"name":name,"byteLength":bytes.len(),"description":"Select a stored page for decoded objects; other payloads are available as raw bytes."})
                    }
                }
            }
            "page" | "layer" | "object" | "replay" | "background" | "replay-svg" => {
                let page_index = number(&r, "page")?;
                let stored = parsed
                    .stored_pages
                    .get(page_index)
                    .ok_or("page out of bounds")?;
                let entry = self.entry_index(&stored.archive_entry)?;
                let bytes = self.entry(entry)?;
                match r["kind"].as_str().unwrap() {
                    "page" => {
                        json!({"entry":entry,"header":stored.page.header,"integrityOffset":stored.page.integrity_offset,"semanticObjects":parsed.document.pages[page_index].objects,"template":parsed.document.pages[page_index].template,"backgroundColor":parsed.document.pages[page_index].background_color,"background":parsed.document.pages[page_index].background,"contentBounds":parsed.document.pages[page_index].content_bbox,"currentLayer":stored.page.layers.current_layer_index,
                        "layers":stored.page.layers.layers.iter().enumerate().map(|(i,l)| json!({"index":i,"number":l.number,"objects":l.objects.len(),"offset":l.header_offset})).collect::<Vec<_>>() })
                    }
                    "layer" => {
                        let layer = stored
                            .page
                            .layers
                            .layers
                            .get(number(&r, "layer")?)
                            .ok_or("layer out of bounds")?;
                        json!({"entry":entry,"offset":layer.header_offset,"headerSize":layer.header_size,
                            "metadataOffset":layer.metadata_offset,"flags":[layer.flags_1,layer.flags_2],"number":layer.number,
                            "headerExtra":layer.header_extra,"integrity":layer.integrity_trailer,
                            "metadata":decoded(layer.metadata_with_limits(bytes,&limits)),"objects":nodes(&layer.objects)})
                    }
                    "object" => {
                        let offset = number(&r, "offset")?;
                        let o = stored
                            .page
                            .layers
                            .layers
                            .iter()
                            .find_map(|l| find(&l.objects, offset))
                            .ok_or("object not found")?;
                        let base = o.base_metadata(bytes);
                        let flexible = match &base {
                            Ok(b) => decoded(b.flexible_metadata_with_limits(&limits)),
                            Err(e) => json!({"error":e.to_string()}),
                        };
                        let metadata = match o.object_type {
                            ObjectType::Stroke => {
                                decoded(o.stroke_metadata_with_limits(bytes, &limits))
                            }
                            ObjectType::Formula => {
                                decoded(o.formula_metadata_with_limits(bytes, &limits))
                            }
                            ObjectType::Plot => {
                                decoded(o.plot_metadata_with_limits(bytes, &limits))
                            }
                            ObjectType::Math => {
                                decoded(o.math_metadata_with_limits(bytes, &limits))
                            }
                            _ => Value::Null,
                        };
                        let stroke = if o.object_type == ObjectType::Stroke {
                            decoded(o.decode_stroke(bytes, &limits).map(|mut stroke| {
                                resources.resolve(&mut stroke);
                                stroke
                            }))
                        } else {
                            Value::Null
                        };
                        json!({"entry":entry,"offset":offset,"size":o.payload_size,"declaredSize":o.declared_size,"type":o.object_type,
                            "integrity":o.integrity_trailer,"base":decoded(base),"flexible":flexible,"metadata":metadata,"stroke":stroke,"objects":nodes(&o.children)})
                    }
                    "background" | "replay-svg" => {
                        let mut preview = layout
                            .pages
                            .iter()
                            .find(|p| p.source_page_index == page_index)
                            .cloned()
                            .unwrap_or_else(|| sdocx::LayoutPage {
                                source_page_index: page_index,
                                body_text: None,
                                page: parsed.document.pages[page_index].clone(),
                            });
                        let replay = r["kind"] == "replay-svg";
                        if !replay {
                            preview.page.clear_strokes();
                        }
                        let preview_layout = LayoutDocument {
                            pages: vec![preview],
                            stored_page_count: layout.stored_page_count,
                            omitted_trailing_blank_page: false,
                        };
                        let mut options = sdocx::RenderOptions::default();
                        options.color_mode = match r["colorMode"].as_str() {
                            Some("dark") => sdocx::RenderColorMode::Dark,
                            Some("light") => sdocx::RenderColorMode::Light,
                            _ => sdocx::RenderColorMode::Auto,
                        };
                        let theme = sdocx::RenderTheme::resolve(
                            &preview_layout.pages[0].page,
                            &parsed.document.metadata,
                            options.color_mode,
                        );
                        let background = if replay {
                            text_cache.render_layout_page_replay_svg(
                                &parsed.document,
                                &preview_layout,
                                0,
                                &options,
                                fonts,
                            )
                        } else {
                            text_cache.render_layout_page_svg(
                                &parsed.document,
                                &preview_layout,
                                0,
                                &options,
                                fonts,
                            )
                        }
                        .ok_or("cannot render page")?
                        .svg;
                        json!({"svg": background,"defaultInk":theme.default_ink()})
                    }
                    _ => {
                        let strokes = parsed.document.pages[page_index]
                            .composed_strokes()
                            .map(|(object, stroke)| json!({
                                "offset": object.source_offset,
                                "geometry": sdocx::prepare_stroke(stroke, false),
                                "stroke": stroke,
                                "milliseconds": stroke.rendering.as_ref().is_some_and(|rendering|
                                    rendering.properties.millisecond_timestamps)
                            }))
                            .collect::<Vec<_>>();
                        let layer = &stored.page.layers.layers
                            [usize::from(stored.page.layers.current_layer_index)];
                        let mut source_objects = HashMap::new();
                        index_objects(&layer.objects, &mut source_objects);
                        let objects = parsed.document.pages[page_index]
                            .composed_objects()
                            .filter_map(|object| {
                                let offset = object.source_offset?;
                                let stored = source_objects.get(&offset)?;
                                let bbox = stored.base_metadata(bytes).ok().map(|base| base.bbox);
                                Some(json!({"offset":offset,"bbox":bbox,"type":stored.object_type}))
                            })
                            .collect::<Vec<_>>();
                        json!({"entry":entry,"width":stored.page.header.width,"height":stored.page.header.height,"strokes":strokes,"objects":objects})
                    }
                }
            }
            _ => return Err("unknown debugger request".into()),
        };
        safe_numbers(&mut result);
        serde_json::to_string(&result).map_err(|e| e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn integers_remain_exact() {
        let mut v = json!({"large":u64::MAX,"modified_time_raw":42,"small":42});
        safe_numbers(&mut v);
        assert_eq!(v["large"], u64::MAX.to_string());
        assert_eq!(v["modified_time_raw"], "42");
        assert_eq!(v["small"], 42);
    }
}

#[cfg(test)]
#[path = "debugger_test_support.rs"]
pub(crate) mod support;

#[cfg(test)]
mod source_tests {
    use super::*;
    use std::io::Write;
    fn fixture() -> Vec<u8> {
        let page = support::page(&[vec![support::object(99, &[1, 2, 3], &[])]], 0, &[]);
        let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
        for (name, data) in [("page.page", page), ("unknown.bin", vec![65; 8193])] {
            writer
                .start_file(name, zip::write::SimpleFileOptions::default())
                .unwrap();
            writer.write_all(&data).unwrap();
        }
        writer.finish().unwrap().into_inner()
    }
    #[test]
    #[ignore = "requires the external conformance corpus"]
    fn replay_and_exports_share_resolved_pen_inputs_and_prepared_geometry() {
        let root = std::env::var("SDOCX_CORPUS_DIR").expect("SDOCX_CORPUS_DIR");
        let bytes =
            std::fs::read(std::path::Path::new(&root).join("02-shapes-and-dot-calibration.sdocx"))
                .unwrap();
        let parsed = sdocx::parse_bytes_detailed(&bytes).unwrap();
        let layout = sdocx::layout_document(&parsed.document);
        let mut source = Source::new(&bytes).unwrap();
        let replay: Value = serde_json::from_str(
            &source
                .request(&parsed, &layout, r#"{"kind":"replay","page":0}"#)
                .unwrap(),
        )
        .unwrap();
        let items = replay["strokes"].as_array().unwrap();
        assert_eq!(items.len(), 77);
        for (item, stroke) in items.iter().zip(parsed.document.pages[0].strokes()) {
            // Use the same Value-to-wire roundtrip as the debugger, including
            // float32 promotion and JSON number parsing.
            let expected: Value = serde_json::from_str(
                &json!({
                    "stroke": stroke,
                    "geometry": sdocx::prepare_stroke(stroke, false)
                })
                .to_string(),
            )
            .unwrap();
            assert_eq!(item["stroke"], expected["stroke"]);
            assert_eq!(item["geometry"], expected["geometry"]);
            assert_eq!(item["geometry"]["profile"], "FountainPen");
            assert_eq!(item["geometry"]["support"], "reconstructed");
        }
    }

    #[test]
    fn lazy_bytes_are_bounded_and_entry_cache_is_replaced() {
        let bytes = fixture();
        let parsed = sdocx::parse_bytes_detailed_with_options(
            &bytes,
            &super::super::browser_parse_options(),
        )
        .unwrap();
        let layout = sdocx::layout_document(&parsed.document);
        let mut source = Source::new(&bytes).unwrap();
        let mut ask = |r: Value| -> Result<Value, String> {
            serde_json::from_str(&source.request(&parsed, &layout, &r.to_string())?)
                .map_err(|e| e.to_string())
        };
        let result = ask(json!({"kind":"bytes","entry":1,"offset":0,"length":10000})).unwrap();
        assert_eq!(result["bytes"].as_array().unwrap().len(), 4096);
        assert!(ask(json!({"kind":"bytes","entry":1,"offset":9000,"length":1})).is_err());
        assert!(ask(json!({"kind":"bytes","entry":99,"offset":0,"length":1})).is_err());
        let end = ask(json!({"kind":"bytes","entry":1,"offset":8192,"length":4096})).unwrap();
        assert_eq!(end["bytes"], json!([65]));
        let raw = ask(json!({"kind":"bytes","entry":null,"offset":0,"length":4})).unwrap();
        assert_eq!(raw["bytes"], json!([80, 75, 3, 4]));
        ask(json!({"kind":"bytes","entry":0,"offset":0,"length":1})).unwrap();
        assert_eq!(source.cache.as_ref().unwrap().0, 0);
    }
    #[test]
    fn replay_borrows_semantic_strokes_instead_of_decoding_the_source_again() {
        let bytes = fixture();
        let mut parsed = sdocx::parse_bytes_detailed(&bytes).unwrap();
        let offset = parsed.stored_pages[0].page.layers.layers[0].objects[0].payload_offset;
        let mut object = sdocx::PageObject::from(sdocx::Stroke {
            rendering: None,
            bbox: sdocx::BoundingBox::default(),
            points: vec![
                sdocx::Point { x: 10., y: 10. },
                sdocx::Point { x: 20., y: 20. },
            ],
            pressures: Vec::new(),
            timestamps: vec![10, 20],
            tilts: Vec::new(),
            orientations: Vec::new(),
            color: Some(sdocx::Color { r: 255, g: 0, b: 0 }),
            pen_width: 2.,
        });
        object.source_offset = Some(offset);
        parsed.document.pages[0].objects.push(object);
        let layout = sdocx::layout_document(&parsed.document);
        let mut source = Source::new(&bytes).unwrap();
        let replay: Value = serde_json::from_str(
            &source
                .request(&parsed, &layout, r#"{"kind":"replay","page":0}"#)
                .unwrap(),
        )
        .unwrap();
        let strokes = replay["strokes"].as_array().unwrap();
        assert_eq!(strokes.len(), 1);
        assert_eq!(strokes[0]["offset"], offset);
        assert_eq!(strokes[0]["stroke"]["color"], json!({"r":255,"g":0,"b":0}));
        assert_eq!(strokes[0]["stroke"]["timestamps"], json!([10, 20]));
        let svg: Value = serde_json::from_str(
            &source
                .request(&parsed, &layout, r#"{"kind":"replay-svg","page":0}"#)
                .unwrap(),
        )
        .unwrap();
        assert!(
            svg["svg"]
                .as_str()
                .unwrap()
                .contains("data-replay-stroke=\"0\"")
        );
        assert!(svg["svg"].as_str().unwrap().contains("#ff0000"));
    }

    #[test]
    fn replay_hit_targets_follow_semantic_selection_and_paint_order() {
        fn base(visible: bool) -> Vec<u8> {
            let fixed = [
                5500_u32.to_le_bytes().to_vec(),
                3_u16.to_le_bytes().to_vec(),
                b"hit".to_vec(),
                0_i64.to_le_bytes().to_vec(),
                [8.0_f64, 8.0, 32.0, 32.0]
                    .into_iter()
                    .flat_map(f64::to_le_bytes)
                    .collect(),
                0_i32.to_le_bytes().to_vec(),
                vec![0],
            ]
            .concat();
            let size = (18 + fixed.len()) as u32;
            [
                size.to_le_bytes().to_vec(),
                0_i16.to_le_bytes().to_vec(),
                size.to_le_bytes().to_vec(),
                vec![2],
                (u16::from(visible) << 3).to_le_bytes().to_vec(),
                vec![4],
                0_u32.to_le_bytes().to_vec(),
                fixed,
            ]
            .concat()
        }
        let bytes = support::archive(&support::page(
            &[vec![
                support::object(99, &base(false), &[support::object(99, &base(true), &[])]),
                support::object(99, &base(true), &[]),
                support::object(99, &base(true), &[]),
            ]],
            0,
            &[],
        ));
        let mut parsed = sdocx::parse_bytes_detailed(&bytes).unwrap();
        let stored = &parsed.stored_pages[0].page.layers.layers[0].objects;
        let offsets = [
            stored[0].payload_offset,
            stored[0].children[0].payload_offset,
            stored[1].payload_offset,
            stored[2].payload_offset,
        ];
        let stroke = sdocx::Stroke {
            rendering: None,
            bbox: sdocx::BoundingBox {
                x_min: 8.,
                y_min: 8.,
                x_max: 32.,
                y_max: 32.,
            },
            points: vec![
                sdocx::Point { x: 10., y: 10. },
                sdocx::Point { x: 20., y: 20. },
            ],
            pressures: Vec::new(),
            timestamps: vec![10, 20],
            tilts: Vec::new(),
            orientations: Vec::new(),
            color: None,
            pen_width: 2.,
        };
        let mut top = sdocx::PageObject::from(stroke.clone());
        top.render_layer = sdocx::ObjectRenderLayer::Top;
        top.source_offset = Some(offsets[2]);
        let mut child = sdocx::PageObject::from(stroke.clone());
        child.source_offset = Some(offsets[1]);
        let mut container = sdocx::PageObject::from(stroke.clone());
        container.source_offset = Some(offsets[0]);
        container.content = sdocx::PageObjectContent::Container(vec![child]);
        let mut rejected = sdocx::PageObject::from(stroke);
        rejected.render_layer = sdocx::ObjectRenderLayer::Other(-1);
        rejected.source_offset = Some(offsets[3]);
        parsed.document.pages[0].objects = vec![top, container, rejected];
        let layout = sdocx::layout_document(&parsed.document);
        let mut source = Source::new(&bytes).unwrap();
        let replay: Value = serde_json::from_str(
            &source
                .request(&parsed, &layout, r#"{"kind":"replay","page":0}"#)
                .unwrap(),
        )
        .unwrap();
        let targets = replay["objects"].as_array().unwrap();
        assert_eq!(
            targets
                .iter()
                .map(|target| target["offset"].as_u64().unwrap())
                .collect::<Vec<_>>(),
            [offsets[0] as u64, offsets[1] as u64, offsets[2] as u64]
        );
        assert!(
            targets
                .iter()
                .all(|target| target["bbox"]
                    == json!({"x_min":8.,"y_min":8.,"x_max":32.,"y_max":32.}))
        );
        assert!(
            targets
                .iter()
                .all(|target| target["type"] == json!({"Other":99}))
        );
        let strokes = replay["strokes"].as_array().unwrap();
        assert_eq!(
            strokes
                .iter()
                .map(|stroke| stroke["offset"].as_u64().unwrap())
                .collect::<Vec<_>>(),
            [offsets[2] as u64, offsets[1] as u64]
        );
        assert!(source.stroke_resources.is_none());
    }

    #[test]
    fn replay_reuses_layout_identity_and_renders_background_only_on_request() {
        let bytes = fixture();
        let mut parsed = sdocx::parse_bytes_detailed_with_options(
            &bytes,
            &super::super::browser_parse_options(),
        )
        .unwrap();
        parsed.document.metadata.default_page_dimensions = Some((1080, 1527));
        parsed.document.metadata.orientation = Some(0);
        parsed.document.pages[0].template = Some(sdocx::PageTemplate {
            id: 7,
            source: sdocx::PageTemplateSource::BuiltIn,
        });
        parsed.document.pages[0].background.width = Some(1080);
        let layout = sdocx::layout_document(&parsed.document);
        let mut source = Source::new(&bytes).unwrap();
        let index: Value = serde_json::from_str(
            &source
                .request(&parsed, &layout, r#"{"kind":"index"}"#)
                .unwrap(),
        )
        .unwrap();
        assert_eq!(index["pages"][0]["visibleIndex"], 0);
        let replay: Value = serde_json::from_str(
            &source
                .request(&parsed, &layout, r#"{"kind":"replay","page":0}"#)
                .unwrap(),
        )
        .unwrap();
        assert!(replay.get("background").is_none());
        let background: Value = serde_json::from_str(
            &source
                .request(
                    &parsed,
                    &layout,
                    r#"{"kind":"background","page":0,"colorMode":"dark"}"#,
                )
                .unwrap(),
        )
        .unwrap();
        let mut options = sdocx::RenderOptions::default();
        options.color_mode = sdocx::RenderColorMode::Dark;
        let expected =
            sdocx::render_layout_page_svg(&parsed.document, &layout, 0, &options).unwrap();
        assert!(expected.svg.contains("data-page-template=\"dots\""));
        let metadata: Value = serde_json::from_str(
            &source
                .request(&parsed, &layout, r#"{"kind":"document"}"#)
                .unwrap(),
        )
        .unwrap();
        assert_eq!(
            metadata["metadata"]["default_page_dimensions"],
            json!([1080, 1527])
        );
        let page: Value = serde_json::from_str(
            &source
                .request(&parsed, &layout, r#"{"kind":"page","page":0}"#)
                .unwrap(),
        )
        .unwrap();
        assert_eq!(page["background"]["width"], 1080);
        assert_eq!(background["svg"], expected.svg);
        assert_eq!(background["defaultInk"], "#ffffff");
    }

    #[test]
    fn unknown_objects_remain_inspectable_with_local_metadata_errors() {
        let bytes = support::archive(&support::page(
            &[vec![support::object(99, &[1, 2, 3], &[])]],
            0,
            &[],
        ));
        let parsed = sdocx::parse_bytes_detailed_with_options(
            &bytes,
            &super::super::browser_parse_options(),
        )
        .unwrap();
        let layout = sdocx::layout_document(&parsed.document);
        let offset = parsed.stored_pages[0].page.layers.layers[0].objects[0].payload_offset;
        let mut source = Source::new(&bytes).unwrap();
        let object: Value = serde_json::from_str(
            &source
                .request(
                    &parsed,
                    &layout,
                    &json!({"kind":"object","page":0,"offset":offset}).to_string(),
                )
                .unwrap(),
        )
        .unwrap();
        assert_eq!(object["offset"], offset);
        assert_eq!(object["size"], 3);
        assert!(object["base"]["error"].is_string());
        assert!(
            source
                .request(&parsed, &layout, r#"{"kind":"object","page":0,"offset":0}"#)
                .is_err()
        );
    }
}

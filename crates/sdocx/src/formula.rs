use crate::binary::Reader;
use crate::frame::Frame;
use crate::math::{checked_payload, read_count, read_latex};
use crate::{
    ArchiveResourceContent, ArchiveResourceError, BoundingBox, DocumentMetadata, Error,
    MathAngleType, ObjectMetadata, ObjectType, ParseLimits, Result, StoredObject, Stroke,
};

#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub struct FormulaMetadata {
    pub base: ObjectMetadata,
    pub has_trigonometry_calculation: bool,
    pub plottable: bool,
    pub latex: Vec<String>,
    pub latex_result_rect: Option<BoundingBox>,
    pub nine_patch_rect: Option<[i32; 4]>,
    pub latex_image_media_id: Option<i32>,
    pub latex_result: Vec<String>,
    pub angle_type: Option<MathAngleType>,
    pub font_size: Option<f32>,
    pub strokes: Vec<FormulaStroke>,
    pub answer_strokes: Vec<FormulaStroke>,
    pub answer: Option<String>,
    pub answer_stroke_color: Option<u32>,
    pub relative_original_formula_rect: Option<BoundingBox>,
    pub relative_original_answer_rect: Option<BoundingBox>,
    pub expression_type_raw: Option<u32>,
    pub label_graphs: Vec<FormulaLabelGraph>,
    pub substitution_latex: Vec<String>,
    pub property_mask: Vec<u8>,
    pub field_mask: Vec<u8>,
    pub fixed_trailing_data: Vec<u8>,
    pub flexible_trailing_data: Vec<u8>,
    pub trailing_data: Vec<u8>,
}

/// The native draw-selection result for a saved Formula, before page placement.
///
/// An image-selected result suppresses both ink lists, even if a later renderer
/// cannot paint the selected bitmap. `result_rect` is the saved destination;
/// exact page placement also needs native `GetDrawnRect` origin recovery.
#[derive(Debug)]
#[allow(dead_code)] // Selection carries exact source inputs for renderer integration.
#[non_exhaustive]
pub(crate) enum FormulaRenderSelection<'a> {
    /// The Formula itself is hidden, so native composition does not draw it.
    Hidden,
    /// A bound raster passed the caller's bitmap-decode probe and wins over ink.
    ImageSelected {
        /// Exact archive path selected by the document media binding.
        resource_name: &'a str,
        /// MIME type recognized by the archive media loader.
        mime_type: &'a str,
        /// Original encoded bytes from the archive.
        data: &'a [u8],
        /// Saved bitmap destination rectangle, if present.
        result_rect: Option<BoundingBox>,
    },
    /// A runtime cache wins after a saved image path is found.
    ///
    /// The cached pixels are runtime-only and are not available from saved
    /// archive source bytes, so this result suppresses ink without supplying
    /// image data for Rust to paint.
    CachedImageSelected {
        /// Saved bitmap destination rectangle, if present.
        result_rect: Option<BoundingBox>,
    },
    /// Native image path or bitmap retrieval failed; ink is returned in draw order.
    StrokeFallback {
        /// Source stroke entries followed by answer stroke entries in stored order.
        strokes: Vec<&'a FormulaStroke>,
    },
    /// Selection cannot be determined safely from the available typed data.
    Unsupported(FormulaRenderUnsupported),
}

/// A reason a Formula image selection cannot safely be reproduced.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub(crate) enum FormulaRenderUnsupported {
    /// The saved image binding points at an ambiguous archive resource.
    AmbiguousImageBinding,
    /// The bound file is retained but is not a supported typed raster asset.
    UnsupportedImageResource,
    /// The bound asset index no longer identifies the selected archive path.
    StaleImageResource,
}

#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub struct FormulaStroke {
    pub base: ObjectMetadata,
    pub stroke: Stroke,
    pub object_data: Vec<u8>,
}

#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub struct FormulaLabelGraph {
    pub labels: Vec<FormulaLabel>,
    pub relations: Vec<FormulaLabelRelation>,
    pub start_label: u32,
    pub end_label: u32,
}

#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub struct FormulaLabel {
    pub text: String,
    pub bbox: BoundingBox,
    pub stroke_indices: Vec<u32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub struct FormulaLabelRelation {
    pub from_label: u32,
    pub to_label: u32,
    pub kind_raw: u32,
}

impl FormulaLabelRelation {
    pub fn kind(&self) -> FormulaLabelRelationKind {
        self.kind_raw.into()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub enum FormulaLabelRelationKind {
    Unknown,
    Right,
    Subscript,
    Superscript,
    Inside,
    Below,
    Above,
    Index,
    Other(u32),
}

impl From<u32> for FormulaLabelRelationKind {
    fn from(value: u32) -> Self {
        match value {
            0 => Self::Unknown,
            1 => Self::Right,
            2 => Self::Subscript,
            3 => Self::Superscript,
            4 => Self::Inside,
            5 => Self::Below,
            6 => Self::Above,
            7 => Self::Index,
            value => Self::Other(value),
        }
    }
}

impl FormulaMetadata {
    /// Select the saved Formula image or its native stroke fallback.
    ///
    /// `runtime_cache_available` reports a cache supplied by a live native
    /// object; saved files do not serialize that bitmap. `bitmap_decodable`
    /// must be the caller's actual raster-backend result for a resolved source
    /// asset. A missing path or failed bitmap probe selects source then answer
    /// stroke entries in stored order. An unresolved decode,
    /// ambiguous binding or unsupported resource returns `Unsupported`
    /// rather than guessing. This resolves draw precedence only; image
    /// placement still needs the native drawn-rect origin, and this method
    /// does not synthesize LaTeX text.
    pub(crate) fn select_render_source<'a>(
        &'a self,
        metadata: &'a DocumentMetadata,
        runtime_cache_available: bool,
        bitmap_decodable: Option<bool>,
    ) -> FormulaRenderSelection<'a> {
        if !self.base.visible {
            return FormulaRenderSelection::Hidden;
        }

        let fallback = || FormulaRenderSelection::StrokeFallback {
            strokes: self.strokes.iter().chain(&self.answer_strokes).collect(),
        };

        let Some(media_id) = self.latex_image_media_id.filter(|id| *id >= 0) else {
            return fallback();
        };
        let resource_name = if let Some(manifest) = &metadata.media_manifest {
            let mut entries = manifest
                .entries
                .iter()
                .filter(|entry| entry.bind_id == media_id as u32);
            let Some(entry) = entries.next() else {
                return fallback();
            };
            if entries.next().is_some() {
                return FormulaRenderSelection::Unsupported(
                    FormulaRenderUnsupported::AmbiguousImageBinding,
                );
            }
            let name = format!("media/{}", entry.file_name);
            if entry.file_name.is_empty() {
                return fallback();
            }
            name
        } else {
            match metadata.resolve_archive_resource(media_id as u32) {
                Ok(resource) => resource.resource.name.clone(),
                Err(ArchiveResourceError::UnboundId { .. }) => return fallback(),
                Err(ArchiveResourceError::MissingEntry { .. }) if !runtime_cache_available => {
                    return fallback();
                }
                Err(
                    ArchiveResourceError::AmbiguousBinding { .. }
                    | ArchiveResourceError::AmbiguousEntry { .. },
                ) => {
                    return FormulaRenderSelection::Unsupported(
                        FormulaRenderUnsupported::AmbiguousImageBinding,
                    );
                }
                Err(
                    ArchiveResourceError::DataUnavailable { .. }
                    | ArchiveResourceError::MissingEntry { .. },
                ) => {
                    return FormulaRenderSelection::Unsupported(
                        FormulaRenderUnsupported::UnsupportedImageResource,
                    );
                }
            }
        };
        let mut resources = metadata
            .archive_resources
            .iter()
            .filter(|resource| resource.name == resource_name);
        let Some(resource) = resources.next() else {
            return if runtime_cache_available {
                FormulaRenderSelection::CachedImageSelected {
                    result_rect: self.latex_result_rect,
                }
            } else {
                fallback()
            };
        };
        if resources.next().is_some() {
            return FormulaRenderSelection::Unsupported(
                FormulaRenderUnsupported::AmbiguousImageBinding,
            );
        }
        let (mime_type, data) = match &resource.content {
            ArchiveResourceContent::MediaAsset { media_index } => {
                match metadata.media_assets.get(*media_index) {
                    Some(asset) if asset.name == resource_name => {
                        (asset.mime_type.as_str(), asset.data.as_slice())
                    }
                    _ if runtime_cache_available => {
                        return FormulaRenderSelection::CachedImageSelected {
                            result_rect: self.latex_result_rect,
                        };
                    }
                    _ => {
                        return FormulaRenderSelection::Unsupported(
                            FormulaRenderUnsupported::StaleImageResource,
                        );
                    }
                }
            }
            ArchiveResourceContent::Opaque { .. } if runtime_cache_available => {
                return FormulaRenderSelection::CachedImageSelected {
                    result_rect: self.latex_result_rect,
                };
            }
            ArchiveResourceContent::Opaque { .. } => {
                return FormulaRenderSelection::Unsupported(
                    FormulaRenderUnsupported::UnsupportedImageResource,
                );
            }
        };
        if runtime_cache_available {
            FormulaRenderSelection::CachedImageSelected {
                result_rect: self.latex_result_rect,
            }
        } else if bitmap_decodable == Some(true) {
            FormulaRenderSelection::ImageSelected {
                resource_name: &resource.name,
                mime_type,
                data,
                result_rect: self.latex_result_rect,
            }
        } else if bitmap_decodable == Some(false) {
            fallback()
        } else {
            FormulaRenderSelection::Unsupported(FormulaRenderUnsupported::UnsupportedImageResource)
        }
    }

    pub fn parse_bytes(bytes: &[u8]) -> Result<Self> {
        Self::parse_bytes_with_limits(bytes, &ParseLimits::default())
    }

    pub fn parse_bytes_with_limits(bytes: &[u8], limits: &ParseLimits) -> Result<Self> {
        if bytes.len() as u64 > limits.max_entry_size {
            return Err(Error::LimitExceeded {
                resource: "formula payload size",
                limit: limits.max_entry_size,
                actual: bytes.len() as u64,
            });
        }
        FormulaDecoder {
            limits,
            entries: 0,
            strokes: 0,
        }
        .parse(bytes)
    }
}

impl StoredObject {
    pub fn formula_metadata(&self, page_bytes: &[u8]) -> Result<FormulaMetadata> {
        self.formula_metadata_with_limits(page_bytes, &ParseLimits::default())
    }

    pub fn formula_metadata_with_limits(
        &self,
        page_bytes: &[u8],
        limits: &ParseLimits,
    ) -> Result<FormulaMetadata> {
        let payload = checked_payload(self, page_bytes, ObjectType::Formula, limits)?;
        FormulaMetadata::parse_bytes_with_limits(payload, limits)
    }
}

struct FormulaDecoder<'a> {
    limits: &'a ParseLimits,
    entries: u64,
    strokes: usize,
}

impl FormulaDecoder<'_> {
    fn parse(mut self, payload: &[u8]) -> Result<FormulaMetadata> {
        let mut reader = Reader::new(payload, "formula object");
        let base = ObjectMetadata::read(&mut reader)?;
        let frame = Frame::read(&mut reader)?;
        frame.expect_kind(11)?;
        let mut flexible = Reader::new(frame.flexible, "formula flexible fields");
        let latex = if frame.fields.contains(0) {
            self.latex_list(&mut flexible)?
        } else {
            Vec::new()
        };
        let latex_result_rect = frame
            .fields
            .contains(1)
            .then(|| crate::object::read_bbox(&mut flexible))
            .transpose()?;
        let nine_patch_rect = frame
            .fields
            .contains(3)
            .then(|| {
                Ok::<_, Error>([
                    flexible.read_i32("nine-patch left")?,
                    flexible.read_i32("nine-patch top")?,
                    flexible.read_i32("nine-patch right")?,
                    flexible.read_i32("nine-patch bottom")?,
                ])
            })
            .transpose()?;
        let latex_image_media_id = frame
            .fields
            .contains(2)
            .then(|| flexible.read_i32("formula image media ID"))
            .transpose()?;
        let latex_result = if frame.fields.contains(4) {
            self.latex_list(&mut flexible)?
        } else {
            Vec::new()
        };
        let angle_type = frame
            .fields
            .contains(5)
            .then(|| flexible.read_u32("formula angle type").map(Into::into))
            .transpose()?;
        let font_size = frame
            .fields
            .contains(6)
            .then(|| flexible.read_f32("formula font size"))
            .transpose()?;
        let strokes = if frame.fields.contains(7) {
            self.stroke_list(&mut flexible)?
        } else {
            Vec::new()
        };
        let answer_strokes = if frame.fields.contains(8) {
            self.stroke_list(&mut flexible)?
        } else {
            Vec::new()
        };
        let answer = frame
            .fields
            .contains(9)
            .then(|| {
                flexible.read_utf16_u16_without_null_sentinel(
                    "formula answer",
                    self.limits.max_text_characters,
                )
            })
            .transpose()?;
        let answer_stroke_color = frame
            .fields
            .contains(10)
            .then(|| flexible.read_u32("formula answer stroke color"))
            .transpose()?;
        let relative_original_formula_rect = frame
            .fields
            .contains(11)
            .then(|| crate::object::read_bbox(&mut flexible))
            .transpose()?;
        let relative_original_answer_rect = frame
            .fields
            .contains(12)
            .then(|| crate::object::read_bbox(&mut flexible))
            .transpose()?;
        let expression_type_raw = frame
            .fields
            .contains(13)
            .then(|| flexible.read_u32("formula expression type"))
            .transpose()?;
        let label_graphs = if frame.fields.contains(14) {
            self.label_graphs(&mut flexible)?
        } else {
            Vec::new()
        };
        let substitution_latex = if frame.fields.contains(15) {
            self.latex_list(&mut flexible)?
        } else {
            Vec::new()
        };
        Ok(FormulaMetadata {
            base,
            has_trigonometry_calculation: frame.properties.contains(0),
            plottable: frame.properties.contains(1),
            latex,
            latex_result_rect,
            nine_patch_rect,
            latex_image_media_id,
            latex_result,
            angle_type,
            font_size,
            strokes,
            answer_strokes,
            answer,
            answer_stroke_color,
            relative_original_formula_rect,
            relative_original_answer_rect,
            expression_type_raw,
            label_graphs,
            substitution_latex,
            property_mask: frame.properties.bytes().to_vec(),
            field_mask: frame.fields.bytes().to_vec(),
            fixed_trailing_data: frame.fixed.to_vec(),
            flexible_trailing_data: flexible
                .read_bytes(flexible.remaining(), "formula flexible trailing data")?
                .to_vec(),
            trailing_data: reader
                .read_bytes(reader.remaining(), "formula trailing data")?
                .to_vec(),
        })
    }

    fn count(&mut self, reader: &mut Reader<'_>, minimum_record_size: usize) -> Result<usize> {
        read_count(reader, minimum_record_size, &mut self.entries, self.limits)
    }

    fn latex_list(&mut self, reader: &mut Reader<'_>) -> Result<Vec<String>> {
        let count = self.count(reader, 2)?;
        let mut values = Vec::with_capacity(count);
        for _ in 0..count {
            values.push(read_latex(reader, self.limits)?);
        }
        Ok(values)
    }

    fn stroke_list(&mut self, reader: &mut Reader<'_>) -> Result<Vec<FormulaStroke>> {
        let count = self.count(reader, 4)?;
        self.strokes = self
            .strokes
            .checked_add(count)
            .ok_or(Error::LimitExceeded {
                resource: "formula strokes",
                limit: self.limits.max_strokes_per_page as u64,
                actual: u64::MAX,
            })?;
        if self.strokes > self.limits.max_strokes_per_page {
            return Err(Error::LimitExceeded {
                resource: "formula strokes",
                limit: self.limits.max_strokes_per_page as u64,
                actual: self.strokes as u64,
            });
        }
        let mut strokes = Vec::with_capacity(count);
        for _ in 0..count {
            let size = reader.read_u32("formula stroke object size")? as usize;
            let data = reader.read_bytes(size, "formula stroke object")?;
            strokes.push(FormulaStroke {
                base: ObjectMetadata::read(&mut Reader::new(data, "formula stroke"))?,
                stroke: crate::decode::decode_stroke(data, self.limits)?,
                object_data: data.to_vec(),
            });
        }
        Ok(strokes)
    }

    fn label_graphs(&mut self, reader: &mut Reader<'_>) -> Result<Vec<FormulaLabelGraph>> {
        let count = self.count(reader, 16)?;
        let mut graphs = Vec::with_capacity(count);
        for _ in 0..count {
            let count = self.count(reader, 40)?;
            let mut labels = Vec::with_capacity(count);
            for _ in 0..count {
                let byte_count = reader.read_u32("formula label byte count")? as usize;
                let bytes = reader.read_bytes(byte_count, "formula label text")?;
                let text = std::str::from_utf8(bytes)
                    .map_err(|_| Error::Format("invalid UTF-8 in formula label".into()))?;
                let units = text.encode_utf16().count();
                if units > self.limits.max_text_characters {
                    return Err(Error::LimitExceeded {
                        resource: "text characters",
                        limit: self.limits.max_text_characters as u64,
                        actual: units as u64,
                    });
                }
                let bbox = crate::object::read_bbox(reader)?;
                let count = self.count(reader, 4)?;
                let mut stroke_indices = Vec::with_capacity(count);
                for _ in 0..count {
                    stroke_indices.push(reader.read_u32("formula label stroke index")?);
                }
                labels.push(FormulaLabel {
                    text: text.to_owned(),
                    bbox,
                    stroke_indices,
                });
            }
            let count = self.count(reader, 12)?;
            let mut relations = Vec::with_capacity(count);
            for _ in 0..count {
                relations.push(FormulaLabelRelation {
                    from_label: reader.read_u32("relation source label")?,
                    to_label: reader.read_u32("relation target label")?,
                    kind_raw: reader.read_u32("relation type")?,
                });
            }
            graphs.push(FormulaLabelGraph {
                labels,
                relations,
                start_label: reader.read_u32("label graph start label")?,
                end_label: reader.read_u32("label graph end label")?,
            });
        }
        Ok(graphs)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ArchiveResource, MediaAsset, MediaManifest, MediaManifestEntry};

    const PNG: &[u8] = include_bytes!("../tests/assets/crop-quadrants.png");

    fn object_base(visible: bool, uuid: &str) -> ObjectMetadata {
        ObjectMetadata {
            format_version: 1,
            uuid: uuid.to_owned(),
            modified_time_raw: 0,
            bbox: BoundingBox::default(),
            replay_timestamp_raw: 0,
            resize_mode_raw: 0,
            rotatable: false,
            selectable: true,
            movable: true,
            visible,
            replayable: false,
            out_of_canvas_enabled: false,
            template: false,
            flip_enabled: false,
            float_drawn_rect: false,
            locked: false,
            removable: true,
            rotation_degrees: None,
            property_mask: Vec::new(),
            field_mask: Vec::new(),
            fixed_trailing_data: Vec::new(),
            flexible_trailing_data: Vec::new(),
        }
    }

    fn formula(media_id: Option<i32>) -> FormulaMetadata {
        FormulaMetadata {
            base: object_base(true, "formula"),
            has_trigonometry_calculation: false,
            plottable: false,
            latex: Vec::new(),
            latex_result_rect: Some(BoundingBox {
                x_min: 10.0,
                y_min: 20.0,
                x_max: 30.0,
                y_max: 40.0,
            }),
            nine_patch_rect: None,
            latex_image_media_id: media_id,
            latex_result: Vec::new(),
            angle_type: None,
            font_size: None,
            strokes: vec![
                formula_stroke(true, "source"),
                formula_stroke(false, "hidden"),
            ],
            answer_strokes: vec![formula_stroke(true, "answer")],
            answer: None,
            answer_stroke_color: None,
            relative_original_formula_rect: None,
            relative_original_answer_rect: None,
            expression_type_raw: None,
            label_graphs: Vec::new(),
            substitution_latex: Vec::new(),
            property_mask: Vec::new(),
            field_mask: Vec::new(),
            fixed_trailing_data: Vec::new(),
            flexible_trailing_data: Vec::new(),
            trailing_data: Vec::new(),
        }
    }

    fn formula_stroke(visible: bool, uuid: &str) -> FormulaStroke {
        FormulaStroke {
            base: object_base(visible, uuid),
            stroke: Stroke {
                rendering: None,
                bbox: BoundingBox::default(),
                points: Vec::new(),
                pressures: Vec::new(),
                timestamps: Vec::new(),
                tilts: Vec::new(),
                orientations: Vec::new(),
                color: None,
                pen_width: 1.0,
            },
            object_data: Vec::new(),
        }
    }

    fn media_document() -> DocumentMetadata {
        let name = "media/latex@abc.png".to_owned();
        DocumentMetadata {
            media_assets: vec![MediaAsset {
                name: name.clone(),
                archive_id: Some(12),
                mime_type: "image/png".to_owned(),
                data: PNG.to_vec(),
            }],
            media_manifest: Some(MediaManifest {
                format_version: 3002,
                entries: vec![MediaManifestEntry {
                    bind_id: 12,
                    file_name: "latex@abc.png".to_owned(),
                    sha256: None,
                    reference_count: 1,
                    modified_time_raw: 0,
                    is_attached: false,
                    trailing_data: Vec::new(),
                }],
                trailing_data: Vec::new(),
            }),
            archive_resources: vec![ArchiveResource {
                name,
                archive_id: Some(12),
                content: ArchiveResourceContent::MediaAsset { media_index: 0 },
            }],
            ..DocumentMetadata::default()
        }
    }

    fn manifest_without_file() -> DocumentMetadata {
        DocumentMetadata {
            media_manifest: Some(MediaManifest {
                format_version: 3002,
                entries: vec![MediaManifestEntry {
                    bind_id: 12,
                    file_name: "latex@abc.png".to_owned(),
                    sha256: None,
                    reference_count: 1,
                    modified_time_raw: 0,
                    is_attached: false,
                    trailing_data: Vec::new(),
                }],
                trailing_data: Vec::new(),
            }),
            ..DocumentMetadata::default()
        }
    }

    fn fallback_ids(selection: FormulaRenderSelection<'_>) -> Vec<String> {
        match selection {
            FormulaRenderSelection::StrokeFallback { strokes } => strokes
                .iter()
                .map(|stroke| stroke.base.uuid.clone())
                .collect(),
            other => panic!("expected stroke fallback, got {other:?}"),
        }
    }

    #[test]
    fn bound_decodable_image_suppresses_both_ink_lists() {
        let formula = formula(Some(12));
        let metadata = media_document();
        let selected = formula.select_render_source(&metadata, false, Some(true));
        match selected {
            FormulaRenderSelection::ImageSelected {
                resource_name,
                result_rect,
                ..
            } => {
                assert_eq!(resource_name, "media/latex@abc.png");
                assert_eq!(result_rect, formula.latex_result_rect);
            }
            other => panic!("expected selected image, got {other:?}"),
        }
    }

    #[test]
    fn missing_binding_uses_source_then_answer_lists_in_native_order() {
        let formula = formula(Some(404));
        let metadata = media_document();
        let selected = formula.select_render_source(&metadata, false, None);
        assert_eq!(fallback_ids(selected), ["source", "hidden", "answer"]);
    }

    #[test]
    fn bitmap_decode_failure_uses_source_then_answer_strokes() {
        let formula = formula(Some(12));
        let metadata = media_document();
        let selected = formula.select_render_source(&metadata, false, Some(false));
        assert_eq!(fallback_ids(selected), ["source", "hidden", "answer"]);
    }

    #[test]
    fn negative_media_id_uses_source_then_answer_strokes() {
        let formula = formula(Some(-1));
        let metadata = media_document();
        let selected = formula.select_render_source(&metadata, false, None);
        assert_eq!(fallback_ids(selected), ["source", "hidden", "answer"]);
    }

    #[test]
    fn path_gate_precedes_runtime_cache_and_cache_precedes_file_decode() {
        let value = formula(Some(12));
        let metadata = manifest_without_file();
        assert!(matches!(
            value.select_render_source(&metadata, false, None),
            FormulaRenderSelection::StrokeFallback { .. }
        ));
        assert!(matches!(
            value.select_render_source(&metadata, true, None),
            FormulaRenderSelection::CachedImageSelected { .. }
        ));

        let unbound = formula(Some(404));
        assert!(matches!(
            unbound.select_render_source(&metadata, true, None),
            FormulaRenderSelection::StrokeFallback { .. }
        ));
    }
}

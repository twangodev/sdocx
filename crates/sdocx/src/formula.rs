use crate::binary::Reader;
use crate::frame::Frame;
use crate::math::{checked_payload, read_count, read_latex};
use crate::{
    BoundingBox, Error, MathAngleType, ObjectMetadata, ObjectType, ParseLimits, Result,
    StoredObject, Stroke,
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

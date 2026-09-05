use crate::binary::Reader;
use crate::frame::Mask;
use crate::{BoundingBox, Error, ParseLimits, Result};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub struct TableRecordMetadata {
    pub property_mask: Vec<u8>,
    pub field_mask: Vec<u8>,
    pub fixed_trailing_data: Vec<u8>,
    pub flexible_trailing_data: Vec<u8>,
}

#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub struct TableStyle {
    pub heading_column_enabled: bool,
    pub heading_row_enabled: bool,
    pub max_height_enabled: bool,
    pub vertical_cell_padding: Option<f32>,
    pub horizontal_cell_padding: Option<f32>,
    pub content_bbox: Option<BoundingBox>,
    pub border: Option<TableBorder>,
    pub auto_fit: Option<TableAutoFit>,
    pub min_column_widths: Option<Vec<f32>>,
    pub max_column_widths: Option<Vec<f32>>,
    pub max_height: Option<f32>,
    pub max_width: Option<f32>,
    pub default_cell_border: Option<TableBorder>,
    pub heading_background_color: Option<u32>,
    pub default_cell_background_color: Option<u32>,
    pub metadata: TableRecordMetadata,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub enum TableAutoFit {
    None,
    Horizontal,
    Vertical,
    Both,
    Other(u8),
}

impl From<u8> for TableAutoFit {
    fn from(value: u8) -> Self {
        match value {
            0 => Self::None,
            1 => Self::Horizontal,
            2 => Self::Vertical,
            3 => Self::Both,
            value => Self::Other(value),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub struct TableBorder {
    pub left: TableEdgeStyle,
    pub top: TableEdgeStyle,
    pub right: TableEdgeStyle,
    pub bottom: TableEdgeStyle,
    pub metadata: TableRecordMetadata,
}

#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub struct TableEdgeStyle {
    pub color: u32,
    pub width: f32,
    pub start_radius: f32,
    pub end_radius: f32,
}

pub(crate) struct TableRecord<'a> {
    pub(crate) properties: Mask<'a>,
    pub(crate) fields: Mask<'a>,
    pub(crate) fixed: Reader<'a>,
    pub(crate) flexible: Reader<'a>,
}

impl<'a> TableRecord<'a> {
    pub(crate) fn read(data: &'a [u8], context: &'static str) -> Result<Self> {
        let mut reader = Reader::new(data, context);
        let flexible_offset = reader.read_u32("table flexible-data offset")? as usize;
        let properties = Mask::read(&mut reader)?;
        let fields = Mask::read(&mut reader)?;
        let fixed_end = if flexible_offset == 0 && !fields.has_other_bits(0) {
            data.len()
        } else {
            flexible_offset
        };
        if fixed_end < reader.position() || fixed_end > data.len() {
            return Err(Error::Format(format!(
                "{context}: table flexible-data offset is outside its record"
            )));
        }
        Ok(Self {
            properties,
            fields,
            fixed: Reader::at(&data[..fixed_end], reader.position(), context)?,
            flexible: Reader::new(&data[fixed_end..], context),
        })
    }

    pub(crate) fn finish(mut self) -> Result<TableRecordMetadata> {
        Ok(TableRecordMetadata {
            property_mask: self.properties.bytes().to_vec(),
            field_mask: self.fields.bytes().to_vec(),
            fixed_trailing_data: self
                .fixed
                .read_bytes(self.fixed.remaining(), "table fixed trailing data")?
                .to_vec(),
            flexible_trailing_data: self
                .flexible
                .read_bytes(self.flexible.remaining(), "table flexible trailing data")?
                .to_vec(),
        })
    }
}

pub(crate) fn read_sized_border(reader: &mut Reader<'_>) -> Result<TableBorder> {
    let size = reader.read_u32("table border size")? as usize;
    let mut record = TableRecord::read(reader.read_bytes(size, "table border")?, "table border")?;
    Ok(TableBorder {
        left: read_edge(&mut record.fixed)?,
        top: read_edge(&mut record.fixed)?,
        right: read_edge(&mut record.fixed)?,
        bottom: read_edge(&mut record.fixed)?,
        metadata: record.finish()?,
    })
}

fn read_edge(reader: &mut Reader<'_>) -> Result<TableEdgeStyle> {
    Ok(TableEdgeStyle {
        color: reader.read_u32("table edge color")?,
        width: reader.read_f32("table edge width")?,
        start_radius: reader.read_f32("table edge start radius")?,
        end_radius: reader.read_f32("table edge end radius")?,
    })
}

pub(crate) fn read_column_widths(
    reader: &mut Reader<'_>,
    limits: &ParseLimits,
) -> Result<Vec<f32>> {
    let count = reader.read_u32("table column count")? as usize;
    if count > limits.max_objects_per_page {
        return Err(Error::LimitExceeded {
            resource: "table columns",
            limit: limits.max_objects_per_page as u64,
            actual: count as u64,
        });
    }
    if count > reader.remaining() / 4 {
        return Err(Error::Format(
            "table column count exceeds its bounded payload".into(),
        ));
    }
    let mut widths = Vec::with_capacity(count);
    for _ in 0..count {
        widths.push(reader.read_f32("table column width")?);
    }
    Ok(widths)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn border() -> Vec<u8> {
        let mut data = vec![0, 0, 0, 0, 1, 0, 2, 0, 0];
        for edge in 0..4_u32 {
            data.extend((0x81112233 + edge).to_le_bytes());
            data.extend((edge as f32 + 1.25).to_le_bytes());
            data.extend((edge as f32 + 5.5).to_le_bytes());
            data.extend((edge as f32 + 9.75).to_le_bytes());
        }
        data
    }

    fn decode(data: &[u8]) -> Result<TableBorder> {
        let bytes = [(data.len() as u32).to_le_bytes().as_slice(), data].concat();
        read_sized_border(&mut Reader::new(&bytes, "test border"))
    }

    #[test]
    fn native_border_order_is_left_top_right_bottom_with_independent_radii() {
        let bytes = border();
        assert_eq!(bytes.len(), 73);
        let parsed = decode(&bytes).unwrap();
        for (index, edge) in [parsed.left, parsed.top, parsed.right, parsed.bottom]
            .into_iter()
            .enumerate()
        {
            assert_eq!(edge.color, 0x81112233 + index as u32);
            assert_eq!(edge.width, index as f32 + 1.25);
            assert_eq!(edge.start_radius, index as f32 + 5.5);
            assert_eq!(edge.end_radius, index as f32 + 9.75);
        }
        assert_eq!(parsed.metadata.property_mask, [0]);
        assert_eq!(parsed.metadata.field_mask, [0, 0]);
        assert!(parsed.metadata.fixed_trailing_data.is_empty());
        assert!(parsed.metadata.flexible_trailing_data.is_empty());
    }

    #[test]
    fn borders_retain_future_masks_and_distinguish_fixed_from_flexible_extensions() {
        let mut bytes = vec![0; 4];
        bytes.extend([5, 0, 0, 0, 0, 1, 5, 0, 0, 0, 0, 2]);
        bytes.extend(&border()[9..]);
        bytes.extend([0xe1, 0xe2, 0xe3]);
        let fixed_end = bytes.len() as u32;
        bytes[..4].copy_from_slice(&fixed_end.to_le_bytes());
        bytes.extend([0xf1, 0xf2]);
        let parsed = decode(&bytes).unwrap();
        assert_eq!(parsed.metadata.property_mask, [0, 0, 0, 0, 1]);
        assert_eq!(parsed.metadata.field_mask, [0, 0, 0, 0, 2]);
        assert_eq!(parsed.metadata.fixed_trailing_data, [0xe1, 0xe2, 0xe3]);
        assert_eq!(parsed.metadata.flexible_trailing_data, [0xf1, 0xf2]);
        assert_eq!(parsed.bottom.width, 4.25);
    }

    #[test]
    fn border_lengths_and_offsets_cannot_borrow_the_following_field() {
        let bytes = border();
        for length in 0..bytes.len() {
            assert!(decode(&bytes[..length]).is_err(), "length {length}");
        }
        for offset in (1..bytes.len()).chain([bytes.len() + 1, u32::MAX as usize]) {
            let mut invalid = bytes.clone();
            invalid[..4].copy_from_slice(&(offset as u32).to_le_bytes());
            assert!(decode(&invalid).is_err(), "offset {offset}");
        }
        let mut bytes = border();
        bytes[7] = 1;
        assert!(decode(&bytes).is_err());
        let mut bytes = 72_u32.to_le_bytes().to_vec();
        bytes.extend(border());
        bytes.extend([0; 100]);
        assert!(read_sized_border(&mut Reader::new(&bytes, "test border")).is_err());
    }

    #[test]
    fn column_vectors_are_bounded_before_allocation() {
        let limits = ParseLimits {
            max_objects_per_page: 2,
            ..Default::default()
        };
        let bytes = [
            2_u32.to_le_bytes(),
            12.5_f32.to_le_bytes(),
            25.5_f32.to_le_bytes(),
        ]
        .concat();
        assert_eq!(
            read_column_widths(&mut Reader::new(&bytes, "columns"), &limits).unwrap(),
            [12.5, 25.5]
        );
        assert!(read_column_widths(&mut Reader::new(&bytes[..8], "columns"), &limits).is_err());
        assert!(matches!(
            read_column_widths(&mut Reader::new(&3_u32.to_le_bytes(), "columns"), &limits),
            Err(Error::LimitExceeded {
                resource: "table columns",
                ..
            })
        ));
    }
}

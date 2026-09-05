use crate::binary::Reader;
use crate::frame::Mask;
use crate::{Error, ParseLimits, Result};

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub struct LayerMetadata {
    pub number: u32,
    pub visible: bool,
    pub event_forwardable: bool,
    pub locked: bool,
    pub alpha_locked: bool,
    pub shadow_visible: bool,
    pub transparency: Option<u8>,
    pub background_color: Option<u32>,
    pub name: Option<String>,
    pub uuid: Option<String>,
    pub modified_time: Option<i64>,
    pub thumbnail_media_id: Option<u32>,
    pub shadow_effect: Option<Vec<u8>>,
    pub property_mask: Vec<u8>,
    pub field_mask: Vec<u8>,
    pub fixed_trailing_data: Vec<u8>,
    pub flexible_trailing_data: Vec<u8>,
}

pub(crate) fn parse_layer_metadata(
    header: &[u8],
    header_offset: usize,
    limits: &ParseLimits,
) -> Result<LayerMetadata> {
    let mut reader = Reader::new(header, "layer metadata header");
    let declared_size = reader.read_u32("header size")? as usize;
    if declared_size != header.len() {
        return Err(Error::Format(
            "layer metadata: inconsistent header size".into(),
        ));
    }
    let absolute_flexible_offset = reader.read_u32("flexible offset")? as usize;
    let properties = Mask::read(&mut reader)?;
    let fields = Mask::read(&mut reader)?;
    let flexible_offset = if absolute_flexible_offset == 0 && !fields.has_other_bits(0) {
        header.len()
    } else {
        absolute_flexible_offset
            .checked_sub(header_offset)
            .ok_or_else(|| {
                Error::Format("layer metadata: flexible offset precedes the layer".into())
            })?
    };
    if flexible_offset < reader.position() || flexible_offset > header.len() {
        return Err(Error::Format(
            "layer metadata: flexible offset is outside the header".into(),
        ));
    }
    let mut fixed = Reader::new(
        &header[reader.position()..flexible_offset],
        "layer fixed fields",
    );
    let number = fixed.read_u32("number")?;
    let mut flexible = Reader::new(&header[flexible_offset..], "layer flexible fields");
    Ok(LayerMetadata {
        number,
        visible: !properties.contains(0),
        event_forwardable: properties.contains(1),
        locked: properties.contains(2),
        alpha_locked: properties.contains(3),
        shadow_visible: properties.contains(4),
        transparency: fields
            .contains(0)
            .then(|| flexible.read_u8("transparency"))
            .transpose()?,
        background_color: fields
            .contains(1)
            .then(|| flexible.read_u32("background color"))
            .transpose()?,
        name: fields
            .contains(2)
            .then(|| flexible.read_utf16_u16_with_limit("name", limits.max_text_characters))
            .transpose()?,
        uuid: fields
            .contains(3)
            .then(|| flexible.read_utf16_u16_with_limit("UUID", limits.max_text_characters))
            .transpose()?,
        modified_time: fields
            .contains(4)
            .then(|| flexible.read_i64("modified time"))
            .transpose()?,
        thumbnail_media_id: fields
            .contains(5)
            .then(|| flexible.read_u32("thumbnail media ID"))
            .transpose()?,
        shadow_effect: fields
            .contains(6)
            .then(|| {
                let length = flexible.read_u32("shadow effect size")? as usize;
                Ok::<_, Error>(flexible.read_bytes(length, "shadow effect")?.to_vec())
            })
            .transpose()?,
        property_mask: properties.bytes().to_vec(),
        field_mask: fields.bytes().to_vec(),
        fixed_trailing_data: fixed
            .read_bytes(fixed.remaining(), "fixed trailing data")?
            .to_vec(),
        flexible_trailing_data: flexible
            .read_bytes(flexible.remaining(), "flexible trailing data")?
            .to_vec(),
    })
}

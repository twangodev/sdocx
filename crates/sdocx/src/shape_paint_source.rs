use crate::Result;
use crate::binary::Reader;

/// A saved paint float, retaining NaN payloads and signed zero.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(transparent))]
pub struct PaintFloat32(u32);

impl PaintFloat32 {
    pub const fn from_bits(bits: u32) -> Self {
        Self(bits)
    }

    pub const fn bits(self) -> u32 {
        self.0
    }

    pub const fn value(self) -> f32 {
        f32::from_bits(self.0)
    }

    fn read(reader: &mut Reader<'_>, field: &'static str) -> Result<Self> {
        reader.read_u32(field).map(Self)
    }

    fn read_array<const N: usize>(
        reader: &mut Reader<'_>,
        field: &'static str,
    ) -> Result<[Self; N]> {
        let mut values = [Self(0); N];
        for value in &mut values {
            *value = Self::read(reader, field)?;
        }
        Ok(values)
    }
}

/// One saved gradient stop; order and repeated positions are significant.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub struct GradientStopSource {
    pub argb: u32,
    pub position: PaintFloat32,
}

/// Color-effect source fields, including gradient settings on solid paints.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub struct ColorPaintSource {
    pub property_flags: u8,
    /// Present only in an outline color record; fill kind uses flags bit 0.
    pub outline_color_type: Option<u8>,
    pub solid_argb: u32,
    pub gradient_type: u8,
    pub linear_angle: u16,
    pub position: [PaintFloat32; 2],
    pub stops: Vec<GradientStopSource>,
    pub trailing_data: Vec<u8>,
}

impl ColorPaintSource {
    pub fn gradient_rotatable(&self) -> bool {
        if self.outline_color_type.is_some() {
            self.property_flags != 0
        } else {
            self.property_flags & 2 != 0
        }
    }

    pub(crate) fn read_body(
        reader: &mut Reader<'_>,
        outline: bool,
        property_flags: u8,
    ) -> Result<Self> {
        let outline_color_type = outline.then(|| reader.read_u8("color type")).transpose()?;
        let solid_argb = reader.read_u32("ARGB")?;
        let gradient_type = reader.read_u8("gradient type")?;
        let linear_angle = reader.read_u16("gradient angle")?;
        let position = [
            PaintFloat32::read(reader, "gradient x")?,
            PaintFloat32::read(reader, "gradient y")?,
        ];
        let count = usize::from(reader.read_u8("gradient stop count")?);
        let mut stops_reader = Reader::new(
            reader.read_bytes(count * 8, "gradient stops")?,
            "gradient stops",
        );
        let mut stops = Vec::with_capacity(count);
        for _ in 0..count {
            stops.push(GradientStopSource {
                argb: stops_reader.read_u32("gradient color")?,
                position: PaintFloat32::read(&mut stops_reader, "gradient stop")?,
            });
        }
        Ok(Self {
            property_flags,
            outline_color_type,
            solid_argb,
            gradient_type,
            linear_angle,
            position,
            stops,
            trailing_data: Vec::new(),
        })
    }
}

/// The saved one-bit repeating tile and its two native ARGB colors.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub struct PatternPaintSource {
    pub rows: [u8; 8],
    pub foreground_argb: u32,
    pub background_argb: u32,
    pub trailing_data: Vec<u8>,
}

impl PatternPaintSource {
    pub(crate) fn read(data: &[u8]) -> Result<Self> {
        let mut reader = Reader::new(data, "shape pattern effect");
        let mut rows = [0; 8];
        rows.copy_from_slice(reader.read_bytes(8, "pattern rows")?);
        let foreground_argb = reader.read_u32("pattern foreground")?;
        let background_argb = reader.read_u32("pattern background")?;
        let trailing_data = reader
            .read_bytes(reader.remaining(), "pattern trailing data")?
            .to_vec();
        Ok(Self {
            rows,
            foreground_argb,
            background_argb,
            trailing_data,
        })
    }
}

/// Saved integer nine-patch coordinates and source width.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub struct NinePatchSource {
    pub coordinates: [i32; 4],
    pub width: i32,
}

/// Ordinary image-fill source fields; no runtime bitmap or resource hash.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub struct ImagePaintSource {
    pub fill_mode: u8,
    pub media_bind_id: i32,
    pub stretch_offsets: [PaintFloat32; 4],
    pub tiling_offsets: [PaintFloat32; 2],
    pub tiling_scales: [PaintFloat32; 2],
    pub transparency: PaintFloat32,
    pub rotatable_byte: u8,
    /// Decoded only when the saved base-object format version is at least 28.
    pub nine_patch: Option<NinePatchSource>,
    pub trailing_data: Vec<u8>,
}

impl ImagePaintSource {
    pub fn rotatable(&self) -> bool {
        self.rotatable_byte != 0
    }

    pub(crate) fn read(data: &[u8], format_version: u32) -> Result<Option<Self>> {
        // Other sizes can contain a coedit hash or an unknown record layout.
        if !matches!(data.len(), 42 | 62) {
            return Ok(None);
        }
        let mut reader = Reader::new(data, "shape image fill");
        let fill_mode = reader.read_u8("image fill mode")?;
        let media_bind_id = reader.read_i32("image fill media ID")?;
        let stretch_offsets = PaintFloat32::read_array(&mut reader, "stretch offset")?;
        let tiling_offsets = PaintFloat32::read_array(&mut reader, "tiling offset")?;
        let tiling_scales = PaintFloat32::read_array(&mut reader, "tiling scale")?;
        let transparency = PaintFloat32::read(&mut reader, "fill transparency")?;
        let rotatable_byte = reader.read_u8("fill rotatable")?;
        let nine_patch = if format_version >= 28 {
            let mut coordinates = [0; 4];
            for coordinate in &mut coordinates {
                *coordinate = reader.read_i32("nine-patch coordinate")?;
            }
            Some(NinePatchSource {
                coordinates,
                width: reader.read_i32("nine-patch width")?,
            })
        } else {
            None
        };
        let trailing_data = reader
            .read_bytes(reader.remaining(), "image fill trailing data")?
            .to_vec();
        Ok(Some(Self {
            fill_mode,
            media_bind_id,
            stretch_offsets,
            tiling_offsets,
            tiling_scales,
            transparency,
            rotatable_byte,
            nine_patch,
            trailing_data,
        }))
    }
}

/// Saved source data independent of the renderer's supported paint projection.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub enum ShapePaintSource {
    Color(ColorPaintSource),
    Pattern(PatternPaintSource),
    Image(ImagePaintSource),
    /// A noncanonical record whose legacy paint projection would drop bytes.
    Opaque {
        data: Vec<u8>,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn color_body_retains_wire_precision_and_requires_complete_stop_bytes() {
        let bytes = [
            0x78, 0x56, 0x34, 0x12, 2, 0xff, 0xff, 0, 0, 0, 0x80, 0x45, 0x23, 0xc1, 0x7f, 1, 0x56,
            0x34, 0x12, 0xaa, 0, 0, 0x80, 0x3f, 0xde, 0xad,
        ];
        let source =
            ColorPaintSource::read_body(&mut Reader::new(&bytes, "color body"), false, 0x82)
                .unwrap();
        assert_eq!(source.property_flags, 0x82);
        assert!(source.gradient_rotatable());
        assert_eq!(source.outline_color_type, None);
        assert_eq!(source.solid_argb, 0x12345678);
        assert_eq!(source.gradient_type, 2);
        assert_eq!(source.linear_angle, u16::MAX);
        assert_eq!(
            source.position.map(PaintFloat32::bits),
            [0x80000000, 0x7fc12345]
        );
        assert_eq!(source.stops[0].argb, 0xaa123456);
        assert_eq!(source.stops[0].position.bits(), 0x3f800000);
        assert!(source.trailing_data.is_empty());
        assert!(
            ColorPaintSource::read_body(
                &mut Reader::new(&bytes[..23], "truncated color body"),
                false,
                0x82
            )
            .is_err()
        );
    }

    #[test]
    fn pattern_retains_source_rows_alpha_and_extension() {
        let mut bytes = vec![0x80, 0x01, 0x55, 0xaa, 0, 0xff, 3, 4];
        bytes.extend(0x12345678_u32.to_le_bytes());
        bytes.extend(0x90abcdef_u32.to_le_bytes());
        bytes.extend([7, 8]);
        let source = PatternPaintSource::read(&bytes).unwrap();
        assert_eq!(source.rows, [0x80, 0x01, 0x55, 0xaa, 0, 0xff, 3, 4]);
        assert_eq!(source.foreground_argb, 0x12345678);
        assert_eq!(source.background_argb, 0x90abcdef);
        assert_eq!(source.trailing_data, [7, 8]);
        assert!(PatternPaintSource::read(&bytes[..15]).is_err());
    }
}

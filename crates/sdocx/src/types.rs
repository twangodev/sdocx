/// A parsed `.sdocx` document containing pages and metadata.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Document {
    /// The pages in the document.
    pub pages: Vec<Page>,
    /// Document-level metadata.
    pub metadata: DocumentMetadata,
}

/// Document-level metadata extracted from the `.sdocx` archive.
#[derive(Debug, Clone, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct DocumentMetadata {
    /// Samsung Notes binary format version recorded by the archive.
    pub format_version: Option<FormatVersion>,
    /// Creation timestamp in milliseconds since the Unix epoch.
    pub created_ms: Option<i64>,
    /// Last modification timestamp in milliseconds since the Unix epoch.
    pub modified_ms: Option<i64>,
    /// Background color of the document.
    pub background_color: Option<Color>,
    /// Whether Samsung Notes dark-mode compatibility is enabled.
    pub dark_mode_compatibility: Option<bool>,
    /// Default page dimensions as `(width, height)` in pixels.
    pub page_dimensions: Option<(u32, u32)>,
    /// Dimensions of the document-level flowing text canvas.
    pub flow_dimensions: Option<(u32, u32)>,
    /// Horizontal and vertical padding used by the flowing text canvas.
    pub flow_page_padding: Option<(u32, u32)>,
    /// Ordered list of page UUIDs.
    pub page_ids: Vec<String>,
    /// Embedded media assets from the archive.
    pub media_assets: Vec<MediaAsset>,
    /// Top-level typed note text from `note.note`, if present.
    pub note_text: Option<RichTextBox>,
    /// Top-level note title from `note.note`, if present.
    pub note_title: Option<RichTextBox>,
}

/// Samsung Notes binary format version.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct FormatVersion(pub u16);

impl FormatVersion {
    /// Earliest version recognized by the current S Pen SDK.
    pub const INITIAL: Self = Self(2034);
    /// Minimum version accepted by current Samsung Notes builds.
    pub const MINIMUM_SUPPORTED: Self = Self(4000);
    /// Version that introduced math objects.
    pub const MATH_OBJECTS: Self = Self(5200);
    /// Version that introduced table and code-block objects.
    pub const TABLE_AND_CODE_BLOCK_OBJECTS: Self = Self(5400);
    /// Current format version exposed by the analyzed SDK.
    pub const CURRENT: Self = Self(5500);

    /// Return the raw numeric format version.
    pub const fn raw(self) -> u16 {
        self.0
    }

    /// Whether this version can contain math objects.
    pub const fn supports_math_objects(self) -> bool {
        self.0 >= Self::MATH_OBJECTS.0
    }

    /// Whether this version can contain table and code-block objects.
    pub const fn supports_table_and_code_block_objects(self) -> bool {
        self.0 >= Self::TABLE_AND_CODE_BLOCK_OBJECTS.0
    }
}

/// A single page within a document.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Page {
    /// Unique identifier for the page.
    pub uuid: String,
    /// Page width in pixels.
    pub width: u32,
    /// Page height in pixels.
    pub height: u32,
    /// Bounding box enclosing all stroke content.
    pub content_bbox: BoundingBox,
    /// Page background color, if present in the page header.
    pub background_color: Option<Color>,
    /// Page template metadata, if present in the page header.
    pub template: Option<PageTemplate>,
    /// The strokes drawn on this page.
    pub strokes: Vec<Stroke>,
    /// Non-stroke page objects parsed from the page stream.
    pub elements: Vec<PageElement>,
}

/// An embedded media asset.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct MediaAsset {
    /// Archive path.
    pub name: String,
    /// Numeric archive resource ID from the filename prefix, when present.
    pub archive_id: Option<u32>,
    /// MIME type, when recognized.
    pub mime_type: String,
    /// Raw media bytes.
    pub data: Vec<u8>,
}

/// A non-stroke page element.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub enum PageElement {
    /// A legacy image value with a caller-supplied asset index.
    Image {
        /// Placement box in page coordinates.
        bbox: BoundingBox,
        /// Index into `DocumentMetadata::media_assets`.
        media_index: usize,
    },
    /// A structurally decoded image with an explicit native media reference.
    PlacedImage(PlacedImage),
    /// A rich text object.
    TextBox(RichTextBox),
    /// A native geometric shape, including its embedded text and styles.
    Shape(crate::NativeShape),
    /// A native line with explicit endpoints and styles.
    Line(crate::NativeLine),
}

impl PageElement {
    /// Return the S Pen SDK object type represented by this element.
    pub const fn object_type(&self) -> ObjectType {
        match self {
            Self::Image { .. } | Self::PlacedImage(_) => ObjectType::Image,
            Self::TextBox(_) => ObjectType::TextBox,
            Self::Shape(_) => ObjectType::Shape,
            Self::Line(_) => ObjectType::Line,
        }
    }
}

/// A native image placement. Unresolved images retain their place in the model.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub struct PlacedImage {
    /// Placement in page coordinates.
    pub bbox: BoundingBox,
    /// Stored clockwise rotation about the placement center.
    pub rotation_degrees: Option<f64>,
    /// Main image bind ID from the image-fill record, excluding native negative sentinels.
    pub media_id: Option<u32>,
    /// Resolved index into `DocumentMetadata::media_assets`, if available and unambiguous.
    pub media_index: Option<usize>,
    /// Stored pixel crop rectangle. Rendering this field is not yet supported.
    pub crop_rect: Option<[i32; 4]>,
    /// Optional border asset ID, distinct from the main image.
    pub border_media_id: Option<u32>,
    /// Optional original asset ID, distinct from the displayed image.
    pub original_media_id: Option<u32>,
}

/// Object type identifiers used by `SpenObjectBase`.
///
/// `Other` preserves identifiers introduced by newer SDKs instead of collapsing
/// them into Samsung's explicit `Unknown` type (`19`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub enum ObjectType {
    /// No object (`0`).
    None,
    /// Pen stroke (`1`).
    Stroke,
    /// Text box (`2`).
    TextBox,
    /// Image (`3`).
    Image,
    /// Object container (`4`).
    Container,
    /// Shape (`7`).
    Shape,
    /// Line (`8`).
    Line,
    /// Deprecated dummy-stroke record (`9`).
    DeprecatedDummyStroke,
    /// Voice recording (`10`).
    Voice,
    /// Formula (`11`).
    Formula,
    /// Deprecated table record (`12`).
    DeprecatedTable,
    /// Web object (`13`).
    Web,
    /// Painting (`14`).
    Painting,
    /// Development-version stroke (`15`).
    StrokeDevelopmentVersion,
    /// Video (`16`).
    Video,
    /// Link (`17`).
    Link,
    /// Brush stroke (`18`).
    StrokeBrush,
    /// Samsung's explicit unknown-object marker (`19`).
    Unknown,
    /// Plot (`20`).
    Plot,
    /// Math object (`21`).
    Math,
    /// Current table object (`22`).
    Table,
    /// Code block (`23`).
    CodeBlock,
    /// Attached file (`24`).
    AttachedFile,
    /// Stroke group (`100`).
    StrokeGroup,
    /// Identifier not known to this version of the library.
    Other(u32),
}

impl ObjectType {
    /// Return the raw `SpenObjectBase` type identifier.
    pub const fn raw(self) -> u32 {
        match self {
            Self::None => 0,
            Self::Stroke => 1,
            Self::TextBox => 2,
            Self::Image => 3,
            Self::Container => 4,
            Self::Shape => 7,
            Self::Line => 8,
            Self::DeprecatedDummyStroke => 9,
            Self::Voice => 10,
            Self::Formula => 11,
            Self::DeprecatedTable => 12,
            Self::Web => 13,
            Self::Painting => 14,
            Self::StrokeDevelopmentVersion => 15,
            Self::Video => 16,
            Self::Link => 17,
            Self::StrokeBrush => 18,
            Self::Unknown => 19,
            Self::Plot => 20,
            Self::Math => 21,
            Self::Table => 22,
            Self::CodeBlock => 23,
            Self::AttachedFile => 24,
            Self::StrokeGroup => 100,
            Self::Other(raw) => raw,
        }
    }

    /// Whether this object type is available in the supplied format version.
    pub const fn is_supported_by(self, version: FormatVersion) -> bool {
        match self {
            Self::Math => version.supports_math_objects(),
            Self::Table | Self::CodeBlock => version.supports_table_and_code_block_objects(),
            _ => true,
        }
    }
}

impl From<u32> for ObjectType {
    fn from(raw: u32) -> Self {
        match raw {
            0 => Self::None,
            1 => Self::Stroke,
            2 => Self::TextBox,
            3 => Self::Image,
            4 => Self::Container,
            7 => Self::Shape,
            8 => Self::Line,
            9 => Self::DeprecatedDummyStroke,
            10 => Self::Voice,
            11 => Self::Formula,
            12 => Self::DeprecatedTable,
            13 => Self::Web,
            14 => Self::Painting,
            15 => Self::StrokeDevelopmentVersion,
            16 => Self::Video,
            17 => Self::Link,
            18 => Self::StrokeBrush,
            19 => Self::Unknown,
            20 => Self::Plot,
            21 => Self::Math,
            22 => Self::Table,
            23 => Self::CodeBlock,
            24 => Self::AttachedFile,
            100 => Self::StrokeGroup,
            raw => Self::Other(raw),
        }
    }
}

impl From<ObjectType> for u32 {
    fn from(object_type: ObjectType) -> Self {
        object_type.raw()
    }
}

/// Parsed rich text box data.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct RichTextBox {
    /// Placement box in page coordinates.
    pub bbox: BoundingBox,
    /// Clockwise rotation in degrees, if present.
    pub rotation_degrees: Option<f64>,
    /// Full text content.
    pub text: String,
    /// Text foreground color.
    pub color: Option<Color>,
    /// Text highlight/fill color.
    pub highlight_color: Option<Color>,
    /// Whether underline styling is present.
    pub underline: bool,
    /// Font size in Samsung Notes logical units, when present.
    pub font_size: Option<f32>,
    /// Style runs using character indexes into `text`.
    pub runs: Vec<RichTextRun>,
    /// Original Samsung style span records.
    pub spans: Vec<RichTextSpan>,
    /// Original Samsung paragraph records.
    pub paragraphs: Vec<RichTextParagraph>,
    /// Embedded objects anchored to U+FFFC replacement characters.
    pub object_spans: Vec<RichTextObjectSpan>,
    /// Per-page UTF-16 text ranges stored by Samsung Notes.
    pub text_sections: Vec<RichTextSection>,
    /// Text margins in left, top, right, bottom order.
    pub margins: Option<[f32; 4]>,
    /// Raw Android text-gravity flags.
    pub gravity: Option<u8>,
}

/// An object embedded into flowing text at a UTF-16 text index.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct RichTextObjectSpan {
    /// Object kind encoded before the object binary.
    pub object_type: ObjectType,
    /// Object's WDoc binary record, retained for type-specific decoding.
    pub object_data: Vec<u8>,
    /// Parsed contents for supported embedded text-object kinds.
    pub content: Option<RichTextObjectContent>,
    /// UTF-16 index of the U+FFFC replacement character.
    pub text_index_utf16: i32,
    /// Inline/block placement behavior.
    pub layout_option: ObjectSpanLayoutOption,
    /// Cross-page placement behavior.
    pub layout_constraint: ObjectSpanLayoutConstraint,
}

/// Parsed contents of an object embedded into flowing text.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub enum RichTextObjectContent {
    /// A Samsung Notes table.
    Table(Box<RichTextTable>),
    /// A Samsung Notes fenced code block.
    CodeBlock(Box<RichTextCodeBlock>),
}

/// A table embedded in flowing note text.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct RichTextTable {
    pub style: crate::TableStyle,
    /// Object placement box stored by the S Pen model.
    pub bbox: BoundingBox,
    /// Clockwise object rotation in degrees, if present.
    pub rotation_degrees: Option<f64>,
    /// Width of each table column in Samsung Notes coordinates.
    pub column_widths: Vec<f32>,
    /// Rows in stored order.
    pub rows: Vec<RichTextTableRow>,
}

/// One row in an embedded table.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct RichTextTableRow {
    pub max_height: Option<f32>,
    pub min_height: Option<f32>,
    pub metadata: crate::TableRecordMetadata,
    /// Stored row index.
    pub index: u32,
    /// Row height in Samsung Notes coordinates.
    pub height: f32,
    /// Cells in stored order.
    pub cells: Vec<RichTextTableCell>,
}

/// One cell in an embedded table.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct RichTextTableCell {
    pub border: Option<crate::TableBorder>,
    pub metadata: crate::TableRecordMetadata,
    /// Stored zero-based column index.
    pub column_index: u32,
    /// Number of rows covered by the cell.
    pub row_span: u32,
    /// Number of columns covered by the cell.
    pub column_span: u32,
    /// Raw Samsung cell background color value.
    pub background_color: u32,
    /// Whether the background color is owned by this cell rather than inherited.
    pub has_own_background_color: bool,
    /// Cell placement box stored by the table model.
    pub bbox: BoundingBox,
    /// Raw Samsung vertical-alignment value.
    pub vertical_alignment: u8,
    /// Rich-text contents of the cell.
    pub content: RichTextBox,
}

/// A fenced code block embedded in flowing note text.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct RichTextCodeBlock {
    /// Object placement box stored by the S Pen model.
    pub bbox: BoundingBox,
    /// Clockwise object rotation in degrees, if present.
    pub rotation_degrees: Option<f64>,
    /// Code-block title or language label.
    pub title: Option<RichTextBox>,
    /// Code-block source text.
    pub body: Option<RichTextBox>,
}

/// Placement option for an object embedded into text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub enum ObjectSpanLayoutOption {
    Block,
    Inline,
    BlockWithSmallMargin,
    BlockWithMediumMargin,
    Other(u32),
}

impl From<u32> for ObjectSpanLayoutOption {
    fn from(raw: u32) -> Self {
        match raw {
            0 => Self::Block,
            1 => Self::Inline,
            2 => Self::BlockWithSmallMargin,
            3 => Self::BlockWithMediumMargin,
            raw => Self::Other(raw),
        }
    }
}

/// Cross-page constraint for an object embedded into text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub enum ObjectSpanLayoutConstraint {
    Normal,
    OverPagesOverlapPadding,
    OverPages,
    Other(u32),
}

impl From<u32> for ObjectSpanLayoutConstraint {
    fn from(raw: u32) -> Self {
        match raw {
            0 => Self::Normal,
            1 => Self::OverPagesOverlapPadding,
            2 => Self::OverPages,
            raw => Self::Other(raw),
        }
    }
}

/// A page's slice of a document-level flowing text object.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct RichTextSection {
    /// Start offset in UTF-16 code units, or `-1` for an empty section.
    pub start_utf16: i32,
    /// Number of UTF-16 code units in the section.
    pub length_utf16: i32,
}

/// A rich text style run.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct RichTextRun {
    /// Start character index, inclusive.
    pub start: usize,
    /// End character index, exclusive.
    pub end: usize,
    /// Whether the run is bold.
    pub bold: bool,
    /// Whether the run is italic.
    pub italic: bool,
}

/// A style span from Samsung's rich-text model.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct RichTextSpan {
    /// Span attribute kind.
    pub kind: RichTextSpanType,
    /// Start offset in UTF-16 code units, inclusive.
    pub start_utf16: u32,
    /// End offset in UTF-16 code units, exclusive.
    pub end_utf16: u32,
    /// Raw span expansion flag.
    pub expand: bool,
    /// Type-specific payload retained for forward compatibility.
    pub payload: Vec<u8>,
}

impl RichTextSpan {
    /// Decode the on/off value used by boolean style spans.
    pub fn boolean_value(&self) -> Option<bool> {
        self.payload
            .get(..2)
            .map(|bytes| u16::from_le_bytes([bytes[0], bytes[1]]) == 1)
    }

    /// Decode a color span's BGRA payload.
    pub fn color_value(&self) -> Option<Color> {
        let bytes = self.payload.get(..4)?;
        Some(Color {
            r: bytes[2],
            g: bytes[1],
            b: bytes[0],
        })
    }

    /// Decode a font-size span's floating-point payload.
    pub fn font_size_value(&self) -> Option<f32> {
        let bytes = self.payload.get(..4)?.try_into().ok()?;
        Some(f32::from_le_bytes(bytes))
    }

    /// Decode the type and optional target stored by a hyperlink span.
    pub fn hyperlink_value(&self) -> Option<RichTextHyperlink> {
        if self.kind != RichTextSpanType::Hyperlink {
            return None;
        }
        let kind = HyperlinkType::from(payload_u32(&self.payload, 0)?);
        let date_time_type = payload_u32(&self.payload, 4)?;
        let length = usize::try_from(payload_u32(&self.payload, 8)?).ok()?;
        let byte_length = length.checked_mul(2)?;
        let bytes = self.payload.get(12..12_usize.checked_add(byte_length)?)?;
        let custom_data = (!bytes.is_empty())
            .then(|| {
                bytes
                    .as_chunks::<2>()
                    .0
                    .iter()
                    .map(|unit| u16::from_le_bytes([unit[0], unit[1]]))
                    .collect::<Vec<_>>()
            })
            .map(|units| String::from_utf16(&units))
            .transpose()
            .ok()?;
        Some(RichTextHyperlink {
            kind,
            date_time_type,
            custom_data,
        })
    }
}

/// Decoded hyperlink metadata from a rich-text span.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct RichTextHyperlink {
    /// Kind of action Samsung associates with the text.
    pub kind: HyperlinkType,
    /// Samsung date/time subtype, meaningful for [`HyperlinkType::DateTime`].
    pub date_time_type: u32,
    /// Explicit target for custom links, if one was stored.
    pub custom_data: Option<String>,
}

/// Samsung hyperlink action identifiers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub enum HyperlinkType {
    Unknown,
    Email,
    Telephone,
    Url,
    Date,
    Address,
    DateTime,
    Formula,
    File,
    Custom,
    Other(u32),
}

impl From<u32> for HyperlinkType {
    fn from(raw: u32) -> Self {
        match raw {
            0 => Self::Unknown,
            1 => Self::Email,
            2 => Self::Telephone,
            3 => Self::Url,
            4 => Self::Date,
            5 => Self::Address,
            6 => Self::DateTime,
            7 => Self::Formula,
            8 => Self::File,
            9 => Self::Custom,
            raw => Self::Other(raw),
        }
    }
}

fn payload_u32(payload: &[u8], offset: usize) -> Option<u32> {
    Some(u32::from_le_bytes(
        payload
            .get(offset..offset.checked_add(4)?)?
            .try_into()
            .ok()?,
    ))
}

/// Samsung rich-text span identifiers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub enum RichTextSpanType {
    /// No style (`0`).
    None,
    /// Foreground color (`1`).
    ForegroundColor,
    /// Font size (`3`).
    FontSize,
    /// Font family name (`4`).
    FontName,
    /// Bold (`5`).
    Bold,
    /// Italic (`6`).
    Italic,
    /// Underline (`7`).
    Underline,
    /// Hyperlink (`9`).
    Hyperlink,
    /// Composition background color (`15`).
    ComposingBackgroundColor,
    /// Composition marker (`16`).
    Composing,
    /// Background/highlight color (`17`).
    BackgroundColor,
    /// Composition tag (`18`).
    ComposingTag,
    /// Timestamp (`19`).
    Timestamp,
    /// Strikethrough (`20`).
    Strikethrough,
    /// Suggestion (`21`).
    Suggestion,
    /// Spell-correction marker (`22`).
    SpellCorrection,
    /// Formula span (`23`).
    Formula,
    /// Identifier not known to this library version.
    Other(u32),
}

impl From<u32> for RichTextSpanType {
    fn from(raw: u32) -> Self {
        match raw {
            0 => Self::None,
            1 => Self::ForegroundColor,
            3 => Self::FontSize,
            4 => Self::FontName,
            5 => Self::Bold,
            6 => Self::Italic,
            7 => Self::Underline,
            9 => Self::Hyperlink,
            15 => Self::ComposingBackgroundColor,
            16 => Self::Composing,
            17 => Self::BackgroundColor,
            18 => Self::ComposingTag,
            19 => Self::Timestamp,
            20 => Self::Strikethrough,
            21 => Self::Suggestion,
            22 => Self::SpellCorrection,
            23 => Self::Formula,
            raw => Self::Other(raw),
        }
    }
}

/// A paragraph attribute record from Samsung's rich-text model.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct RichTextParagraph {
    /// Paragraph attribute kind.
    pub kind: RichTextParagraphType,
    /// Start paragraph ordinal, inclusive.
    pub start_paragraph: u32,
    /// End paragraph ordinal, exclusive.
    pub end_paragraph: u32,
    /// Type-specific payload retained for forward compatibility.
    pub payload: Vec<u8>,
}

impl RichTextParagraph {
    /// Decode an alignment paragraph's value.
    pub fn alignment(&self) -> Option<ParagraphAlignment> {
        if self.kind != RichTextParagraphType::Alignment {
            return None;
        }
        Some(ParagraphAlignment::from(self.payload_u32(0)?))
    }

    /// Decode an indentation paragraph's level and writing direction.
    pub fn indent(&self) -> Option<ParagraphIndent> {
        if self.kind != RichTextParagraphType::IndentLevel {
            return None;
        }
        Some(ParagraphIndent {
            level: self.payload_u32(0)?,
            direction: ParagraphDirection::from(self.payload_u32(4)?),
        })
    }

    /// Decode a line-spacing paragraph's unit and value.
    pub fn line_spacing(&self) -> Option<ParagraphLineSpacing> {
        if self.kind != RichTextParagraphType::LineSpacing {
            return None;
        }
        Some(ParagraphLineSpacing {
            kind: LineSpacingType::from(self.payload_u32(0)?),
            value: self.payload_f32(4)?,
        })
    }

    /// Decode a bullet, numbered-list, or checkbox paragraph.
    pub fn bullet(&self) -> Option<ParagraphBullet> {
        if self.kind != RichTextParagraphType::Bullet {
            return None;
        }
        Some(ParagraphBullet {
            kind: BulletType::from(self.payload_u32(0)?),
            number: self.payload_u32(4)?,
            checked: self.payload_u32(8)? != 0,
            initial_number: self.payload_u32(12)?,
        })
    }

    /// Decode spacing before or after a paragraph, in logical pixels.
    pub fn spacing(&self) -> Option<f32> {
        matches!(
            self.kind,
            RichTextParagraphType::SpacingBefore | RichTextParagraphType::SpacingAfter
        )
        .then(|| self.payload_f32(0))?
    }

    /// Decode a predefined heading/body style and the style for the next paragraph.
    pub fn predefined_style(&self) -> Option<ParagraphPredefinedStyle> {
        if self.kind != RichTextParagraphType::PredefinedStyle {
            return None;
        }
        Some(ParagraphPredefinedStyle {
            style: PredefinedTextStyle::from(self.payload_u32(0)?),
            following_style: PredefinedTextStyle::from(self.payload_u32(4)?),
        })
    }

    /// Decode whether Samsung's Markdown parser has processed this paragraph.
    pub fn is_parsed(&self) -> Option<bool> {
        (self.kind == RichTextParagraphType::ParsingState)
            .then(|| self.payload_u32(0).map(|value| value != 0))?
    }

    fn payload_u32(&self, offset: usize) -> Option<u32> {
        let bytes = self.payload.get(offset..offset.checked_add(4)?)?;
        Some(u32::from_le_bytes(bytes.try_into().ok()?))
    }

    fn payload_f32(&self, offset: usize) -> Option<f32> {
        let bytes = self.payload.get(offset..offset.checked_add(4)?)?;
        Some(f32::from_le_bytes(bytes.try_into().ok()?))
    }
}

/// Samsung paragraph attribute identifiers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub enum RichTextParagraphType {
    /// No paragraph attribute (`0`).
    None,
    /// Legacy paragraph attribute (`1`).
    Legacy,
    /// Indentation level (`2`).
    IndentLevel,
    /// Text alignment (`3`).
    Alignment,
    /// Line spacing (`4`).
    LineSpacing,
    /// Bullet, numbered-list, or checkbox state (`5`).
    Bullet,
    /// Markdown/parsing state (`6`).
    ParsingState,
    /// Spacing before the paragraph (`8`).
    SpacingBefore,
    /// Spacing after the paragraph (`9`).
    SpacingAfter,
    /// Predefined heading/body style (`10`).
    PredefinedStyle,
    /// Identifier not known to this library version.
    Other(u32),
}

impl From<u32> for RichTextParagraphType {
    fn from(raw: u32) -> Self {
        match raw {
            0 => Self::None,
            1 => Self::Legacy,
            2 => Self::IndentLevel,
            3 => Self::Alignment,
            4 => Self::LineSpacing,
            5 => Self::Bullet,
            6 => Self::ParsingState,
            8 => Self::SpacingBefore,
            9 => Self::SpacingAfter,
            10 => Self::PredefinedStyle,
            raw => Self::Other(raw),
        }
    }
}

/// Horizontal paragraph alignment.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub enum ParagraphAlignment {
    Left,
    Right,
    Center,
    Both,
    Other(u32),
}

impl From<u32> for ParagraphAlignment {
    fn from(raw: u32) -> Self {
        match raw {
            0 => Self::Left,
            1 => Self::Right,
            2 => Self::Center,
            3 => Self::Both,
            raw => Self::Other(raw),
        }
    }
}

/// Writing direction attached to an indentation paragraph.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub enum ParagraphDirection {
    None,
    LeftToRight,
    RightToLeft,
    Other(u32),
}

impl From<u32> for ParagraphDirection {
    fn from(raw: u32) -> Self {
        match raw {
            0 => Self::None,
            1 => Self::LeftToRight,
            2 => Self::RightToLeft,
            raw => Self::Other(raw),
        }
    }
}

/// Decoded indentation paragraph value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ParagraphIndent {
    /// Zero-based nesting level.
    pub level: u32,
    /// Writing direction for the paragraph.
    pub direction: ParagraphDirection,
}

/// Unit used by Samsung's line-spacing paragraph value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub enum LineSpacingType {
    Pixels,
    Percent,
    Other(u32),
}

impl From<u32> for LineSpacingType {
    fn from(raw: u32) -> Self {
        match raw {
            0 => Self::Pixels,
            1 => Self::Percent,
            raw => Self::Other(raw),
        }
    }
}

/// Decoded line-spacing paragraph value.
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ParagraphLineSpacing {
    /// Whether `value` is expressed in pixels or as a multiplier.
    pub kind: LineSpacingType,
    /// Pixel distance or proportional multiplier.
    pub value: f32,
}

/// Samsung Notes list marker type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub enum BulletType {
    None,
    Arrow,
    Checker,
    Diamond,
    Digit,
    CircledDigit,
    Alphabet,
    RomanNumeral,
    SolidCircle,
    WhiteCircle,
    UppercaseAlphabet,
    BlackSquare,
    WhiteSquare,
    Other(u32),
}

impl From<u32> for BulletType {
    fn from(raw: u32) -> Self {
        match raw {
            0 => Self::None,
            1 => Self::Arrow,
            2 => Self::Checker,
            3 => Self::Diamond,
            4 => Self::Digit,
            5 => Self::CircledDigit,
            6 => Self::Alphabet,
            7 => Self::RomanNumeral,
            8 => Self::SolidCircle,
            9 => Self::WhiteCircle,
            10 => Self::UppercaseAlphabet,
            11 => Self::BlackSquare,
            12 => Self::WhiteSquare,
            raw => Self::Other(raw),
        }
    }
}

/// Decoded bullet, numbered-list, or checkbox paragraph value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ParagraphBullet {
    /// List marker kind.
    pub kind: BulletType,
    /// Current sequence number for numbered markers.
    pub number: u32,
    /// Checkbox state when the marker is a task item.
    pub checked: bool,
    /// Initial sequence number for numbered markers.
    pub initial_number: u32,
}

/// Samsung Notes predefined heading/body style.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub enum PredefinedTextStyle {
    Heading1,
    Heading2,
    Heading3,
    Body1,
    Other(u32),
}

impl From<u32> for PredefinedTextStyle {
    fn from(raw: u32) -> Self {
        match raw {
            0 => Self::Heading1,
            1 => Self::Heading2,
            2 => Self::Heading3,
            3 => Self::Body1,
            raw => Self::Other(raw),
        }
    }
}

/// Decoded predefined style and the style Samsung applies after Enter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ParagraphPredefinedStyle {
    /// Style applied to this paragraph.
    pub style: PredefinedTextStyle,
    /// Style Samsung applies to the next paragraph after Enter.
    pub following_style: PredefinedTextStyle,
}

/// Page template metadata.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PageTemplate {
    /// Raw Samsung Notes template identifier.
    pub id: u32,
    /// Template backing source.
    pub source: PageTemplateSource,
}

/// Page template backing source.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub enum PageTemplateSource {
    /// Built-in Samsung Notes page template.
    BuiltIn,
    /// Custom PDF-backed page template.
    CustomPdf {
        /// Zero-based PDF page index used as the template.
        page_index: u32,
    },
}

/// A single pen stroke consisting of points and associated data.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Stroke {
    /// Bounding box of the stroke.
    pub bbox: BoundingBox,
    /// The (x, y) coordinates along the stroke path.
    pub points: Vec<Point>,
    /// Pressure values for each point as exposed by the S Pen SDK.
    pub pressures: Vec<f64>,
    /// Per-point event timestamps in Samsung's native units.
    pub timestamps: Vec<i64>,
    /// Stylus tilt values for each point when stream metadata identifies the channel.
    pub tilts: Vec<f64>,
    /// Stylus orientation values for each point when stream metadata identifies the channel.
    pub orientations: Vec<f64>,
    /// Stroke color, if present.
    pub color: Option<Color>,
    /// Pen width in pixels.
    pub pen_width: f32,
}

impl Stroke {
    /// Return the S Pen SDK object type represented by a parsed stroke.
    pub const fn object_type(&self) -> ObjectType {
        ObjectType::Stroke
    }
}

/// A 2D point.
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Point {
    /// X coordinate.
    pub x: f64,
    /// Y coordinate.
    pub y: f64,
}

/// An RGB color.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Color {
    /// Red channel (0–255).
    pub r: u8,
    /// Green channel (0–255).
    pub g: u8,
    /// Blue channel (0–255).
    pub b: u8,
}

/// An axis-aligned bounding box.
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct BoundingBox {
    /// Minimum X coordinate.
    pub x_min: f64,
    /// Minimum Y coordinate.
    pub y_min: f64,
    /// Maximum X coordinate.
    pub x_max: f64,
    /// Maximum Y coordinate.
    pub y_max: f64,
}

impl Default for BoundingBox {
    fn default() -> Self {
        Self {
            x_min: 0.0,
            y_min: 0.0,
            x_max: 0.0,
            y_max: 0.0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        BulletType, FormatVersion, HyperlinkType, LineSpacingType, ObjectType, ParagraphAlignment,
        ParagraphDirection, PredefinedTextStyle, RichTextParagraph, RichTextParagraphType,
        RichTextSpan, RichTextSpanType,
    };

    fn paragraph(kind: RichTextParagraphType, values: &[u32]) -> RichTextParagraph {
        RichTextParagraph {
            kind,
            start_paragraph: 0,
            end_paragraph: 1,
            payload: values
                .iter()
                .flat_map(|value| value.to_le_bytes())
                .collect(),
        }
    }

    #[test]
    fn object_type_ids_round_trip_without_losing_future_values() {
        for raw in 0..=100 {
            let object_type = ObjectType::from(raw);
            assert_eq!(object_type.raw(), raw);
        }
        assert_eq!(ObjectType::from(19), ObjectType::Unknown);
        assert_eq!(ObjectType::from(25), ObjectType::Other(25));
    }

    #[test]
    fn version_gates_match_current_sdk_contract() {
        assert!(!ObjectType::Math.is_supported_by(FormatVersion(5199)));
        assert!(ObjectType::Math.is_supported_by(FormatVersion::MATH_OBJECTS));
        assert!(!ObjectType::Table.is_supported_by(FormatVersion(5399)));
        assert!(ObjectType::Table.is_supported_by(FormatVersion::TABLE_AND_CODE_BLOCK_OBJECTS));
        assert!(ObjectType::Stroke.is_supported_by(FormatVersion::INITIAL));
    }

    #[test]
    fn decodes_apk_hyperlink_payload() {
        let target = "https://example.com/markdown-test";
        let mut payload = Vec::new();
        payload.extend_from_slice(&3_u32.to_le_bytes());
        payload.extend_from_slice(&0_u32.to_le_bytes());
        payload.extend_from_slice(&(target.encode_utf16().count() as u32).to_le_bytes());
        payload.extend(target.encode_utf16().flat_map(|unit| unit.to_le_bytes()));
        let span = RichTextSpan {
            kind: RichTextSpanType::Hyperlink,
            start_utf16: 0,
            end_utf16: 12,
            expand: false,
            payload,
        };

        let hyperlink = span.hyperlink_value().unwrap();
        assert_eq!(hyperlink.kind, HyperlinkType::Url);
        assert_eq!(hyperlink.date_time_type, 0);
        assert_eq!(hyperlink.custom_data.as_deref(), Some(target));
    }

    #[test]
    fn decodes_apk_paragraph_payloads() {
        assert_eq!(
            paragraph(RichTextParagraphType::Alignment, &[2]).alignment(),
            Some(ParagraphAlignment::Center)
        );

        let indent = paragraph(RichTextParagraphType::IndentLevel, &[3, 2])
            .indent()
            .unwrap();
        assert_eq!(indent.level, 3);
        assert_eq!(indent.direction, ParagraphDirection::RightToLeft);

        let line_spacing = paragraph(RichTextParagraphType::LineSpacing, &[1, 1.6_f32.to_bits()])
            .line_spacing()
            .unwrap();
        assert_eq!(line_spacing.kind, LineSpacingType::Percent);
        assert_eq!(line_spacing.value, 1.6);

        let bullet = paragraph(RichTextParagraphType::Bullet, &[4, 2, 1, 1])
            .bullet()
            .unwrap();
        assert_eq!(bullet.kind, BulletType::Digit);
        assert_eq!(bullet.number, 2);
        assert!(bullet.checked);
        assert_eq!(bullet.initial_number, 1);

        assert_eq!(
            paragraph(RichTextParagraphType::SpacingBefore, &[20.0_f32.to_bits()]).spacing(),
            Some(20.0)
        );

        let style = paragraph(RichTextParagraphType::PredefinedStyle, &[0, 3])
            .predefined_style()
            .unwrap();
        assert_eq!(style.style, PredefinedTextStyle::Heading1);
        assert_eq!(style.following_style, PredefinedTextStyle::Body1);

        assert_eq!(
            paragraph(RichTextParagraphType::ParsingState, &[1]).is_parsed(),
            Some(true)
        );
    }
}

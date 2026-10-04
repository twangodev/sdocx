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
    /// Native default page size stored after the note body (not the flow canvas).
    #[cfg_attr(feature = "serde", serde(default))]
    pub default_page_dimensions: Option<(u32, u32)>,
    /// Raw native page mode: 0 is a page list, 1 is a continuous canvas.
    #[cfg_attr(feature = "serde", serde(default))]
    pub page_mode: Option<u16>,
    /// Raw document orientation: 0 portrait, 1 landscape.
    #[cfg_attr(feature = "serde", serde(default))]
    pub orientation: Option<i32>,
    /// Native body font-size adjustment; `i32::MIN` requests the device default.
    #[cfg_attr(feature = "serde", serde(default))]
    pub body_font_size_delta: Option<i32>,
    /// Dimensions of the document-level flowing text canvas.
    pub flow_dimensions: Option<(u32, u32)>,
    /// Horizontal and vertical padding used by the flowing text canvas.
    pub flow_page_padding: Option<(u32, u32)>,
    /// Ordered list of page UUIDs.
    pub page_ids: Vec<String>,
    /// Embedded raster image assets from the archive.
    pub media_assets: Vec<MediaAsset>,
    /// Authoritative media bindings, including entries whose files are missing or unsupported.
    #[cfg_attr(feature = "serde", serde(default))]
    pub media_manifest: Option<crate::MediaManifest>,
    /// Retained `media/` source files; opaque resources are not image render inputs.
    #[cfg_attr(feature = "serde", serde(default))]
    pub archive_resources: Vec<crate::ArchiveResource>,
    /// Top-level typed note text from `note.note`, if present.
    pub note_text: Option<RichTextBox>,
    /// Top-level note title from `note.note`, if present.
    pub note_title: Option<RichTextBox>,
}

impl DocumentMetadata {
    /// Scale logical text units from the oriented native default page size.
    pub fn document_density(&self) -> f32 {
        let Some((width, height)) = self.default_page_dimensions else {
            return 1.0;
        };
        let axis = if self.orientation.unwrap_or(0) == 0 {
            width
        } else {
            height
        };
        let density = axis as i32 as f32 / 360.0;
        if density <= 0.0 { 1.0 } else { density }
    }
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
    /// Raw background settings retained for inspection and template validation.
    #[cfg_attr(feature = "serde", serde(default))]
    pub background: PageBackground,

    /// Page objects in stored order, retaining container boundaries.
    pub objects: Vec<crate::PageObject>,
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
#[derive(Debug, Clone, PartialEq)]
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
    pub crop_rect: Option<[i32; 4]>,
    pub original_bbox: Option<BoundingBox>,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub enum TextAreaType {
    Margin,
    Free,
    Path,
    Other(u8),
}

impl From<u8> for TextAreaType {
    fn from(raw: u8) -> Self {
        match raw {
            0 => Self::Margin,
            1 => Self::Free,
            2 => Self::Path,
            raw => Self::Other(raw),
        }
    }
}

impl TextAreaType {
    pub const fn raw(self) -> u8 {
        match self {
            Self::Margin => 0,
            Self::Free => 1,
            Self::Path => 2,
            Self::Other(raw) => raw,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct RichTextBox {
    pub text_area_type: Option<TextAreaType>,
    /// Placement box in page coordinates.
    pub bbox: BoundingBox,
    /// Clockwise rotation in degrees, if present.
    pub rotation_degrees: Option<f64>,
    /// Full text content.
    pub text: String,
    /// Whole-text foreground color; partial colors remain in `spans`.
    pub color: Option<Color>,
    /// Whole-text highlight/fill color; partial highlights remain in `spans`.
    pub highlight_color: Option<Color>,
    /// Whole-text underline default, overridden by range-specific `spans`.
    pub underline: bool,
    /// Whole-text font size in Samsung Notes logical units, when present.
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
    /// Raw vertical text gravity: 0 top, 1 center, 2 bottom.
    pub gravity: Option<u8>,
}

/// An object embedded into flowing text at a UTF-16 text index.
#[derive(Debug, Clone, PartialEq)]
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

impl RichTextObjectSpan {
    /// Decode the common metadata from the retained WDoc object record.
    pub fn object_metadata(&self) -> crate::Result<Option<crate::ObjectMetadata>> {
        if self.object_data.is_empty() {
            return Ok(None);
        }
        let mut reader = crate::binary::Reader::new(&self.object_data, "rich-text object metadata");
        crate::ObjectMetadata::read(&mut reader).map(Some)
    }
}

/// Parsed contents of an object embedded into flowing text.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub enum RichTextObjectContent {
    Image(Box<PlacedImage>),
    /// A Samsung Notes table.
    Table(Box<RichTextTable>),
    /// A Samsung Notes fenced code block.
    CodeBlock(Box<RichTextCodeBlock>),
}

/// A table embedded in flowing note text.
#[derive(Debug, Clone, PartialEq)]
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
#[derive(Debug, Clone, PartialEq)]
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
#[derive(Debug, Clone, PartialEq)]
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
    /// Whether the cell contents can be edited.
    pub editable: bool,
    /// Rich-text contents of the cell.
    pub content: RichTextBox,
}

/// A fenced code block embedded in flowing note text.
#[derive(Debug, Clone, PartialEq)]
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
#[derive(Debug, Clone, PartialEq)]
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

/// Native endpoint policy for caret lookup in a rich-text span.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SpanIntervalType {
    /// Includes the start and excludes the end; an empty interval includes its start.
    #[default]
    ClosedOpen,
    /// Includes both endpoints.
    ClosedClosed,
    /// Excludes both endpoints.
    OpenOpen,
    /// Excludes the start and includes the end; an empty interval includes its end.
    OpenClosed,
    /// Unrecognized native value, using the native open/closed fallback policy.
    Other(u32),
}

impl SpanIntervalType {
    /// Return the unchanged native interval value.
    pub const fn raw(self) -> u32 {
        match self {
            Self::ClosedOpen => 0,
            Self::ClosedClosed => 1,
            Self::OpenOpen => 2,
            Self::OpenClosed => 3,
            Self::Other(raw) => raw,
        }
    }

    /// Apply native caret lookup semantics to UTF-16 offsets.
    pub const fn contains_caret(self, start: u32, end: u32, index: u32) -> bool {
        match self {
            Self::ClosedOpen => index == start || start < index && index < end,
            Self::ClosedClosed => start <= index && index <= end,
            Self::OpenOpen => start < index && index < end,
            Self::OpenClosed | Self::Other(_) => index == end || start < index && index < end,
        }
    }
}

impl From<u32> for SpanIntervalType {
    fn from(raw: u32) -> Self {
        match raw {
            0 => Self::ClosedOpen,
            1 => Self::ClosedClosed,
            2 => Self::OpenOpen,
            3 => Self::OpenClosed,
            raw => Self::Other(raw),
        }
    }
}

#[cfg(feature = "serde")]
impl serde::Serialize for SpanIntervalType {
    fn serialize<S: serde::Serializer>(
        &self,
        serializer: S,
    ) -> std::result::Result<S::Ok, S::Error> {
        serializer.serialize_u32(self.raw())
    }
}

#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for SpanIntervalType {
    fn deserialize<D: serde::Deserializer<'de>>(
        deserializer: D,
    ) -> std::result::Result<Self, D::Error> {
        #[derive(serde::Deserialize)]
        #[serde(untagged)]
        enum Value {
            Raw(u32),
            Legacy(bool),
        }
        Ok(Self::from(match Value::deserialize(deserializer)? {
            Value::Raw(raw) => raw,
            Value::Legacy(expand) => u32::from(expand),
        }))
    }
}

/// A style span from Samsung's rich-text model.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct RichTextSpan {
    /// Span attribute kind.
    pub kind: RichTextSpanType,
    /// Start offset in UTF-16 code units.
    pub start_utf16: u32,
    /// End offset in UTF-16 code units.
    pub end_utf16: u32,
    /// Native endpoint policy, retaining unknown values.
    #[cfg_attr(feature = "serde", serde(alias = "expand"))]
    pub interval_type: SpanIntervalType,
    /// Type-specific payload retained for forward compatibility.
    pub payload: Vec<u8>,
}

impl RichTextSpan {
    /// Whether native caret lookup includes this UTF-16 position.
    pub const fn contains_caret(&self, index_utf16: u32) -> bool {
        self.interval_type
            .contains_caret(self.start_utf16, self.end_utf16, index_utf16)
    }

    /// Decode the on/off value used by boolean style spans.
    pub fn boolean_value(&self) -> Option<bool> {
        self.payload
            .get(..2)
            .map(|bytes| u16::from_le_bytes([bytes[0], bytes[1]]) == 1)
    }

    /// Decode a color span's BGRA payload while retaining alpha.
    pub fn argb_value(&self) -> Option<u32> {
        Some(u32::from_le_bytes(self.payload.get(..4)?.try_into().ok()?))
    }

    /// Decode a color span's RGB channels.
    pub fn color_value(&self) -> Option<Color> {
        let argb = self.argb_value()?;
        Some(Color {
            r: (argb >> 16) as u8,
            g: (argb >> 8) as u8,
            b: argb as u8,
        })
    }

    /// Decode a composing or composing-tag flag from its modern eight-byte payload.
    pub fn composition_value(&self) -> Option<bool> {
        if !matches!(
            self.kind,
            RichTextSpanType::Composing | RichTextSpanType::ComposingTag
        ) {
            return None;
        }
        Some(self.payload.get(..8)?[0] != 0)
    }

    /// Decode composing-background ARGB from its modern eight-byte payload.
    pub fn composing_background_value(&self) -> Option<u32> {
        if self.kind != RichTextSpanType::ComposingBackgroundColor {
            return None;
        }
        payload_u32(self.payload.get(..8)?, 0)
    }

    /// Decode suggestion metadata, retaining raw identifiers and omitting empty entries.
    /// Malformed UTF-16 or a truncated declared list has no decoded value.
    pub fn suggestion_value(&self) -> Option<RichTextSuggestion> {
        if self.kind != RichTextSpanType::Suggestion {
            return None;
        }
        let mut reader = crate::binary::Reader::new(&self.payload, "suggestion span");
        let suggestion_type = reader.read_u32("suggestion type").ok()?;
        let underline_argb = reader.read_u32("underline ARGB").ok()?;
        let count = reader.read_i32("suggestion count").ok()?;
        let count = usize::try_from(count).unwrap_or(0);
        if count > reader.remaining() / 2 {
            return None;
        }
        let mut strings = Vec::new();
        for _ in 0..count {
            let text = reader
                .read_utf16_u16_without_null_sentinel("suggestion", usize::MAX)
                .ok()?;
            if !text.is_empty() {
                strings.push(text);
            }
        }
        Some(RichTextSuggestion {
            suggestion_type,
            underline_argb,
            strings,
        })
    }

    /// Decode a font-size span's floating-point payload.
    pub fn font_size_value(&self) -> Option<f32> {
        let bytes = self.payload.get(..4)?.try_into().ok()?;
        Some(f32::from_le_bytes(bytes))
    }

    /// Decode a font-name span's length-prefixed, NUL-terminated UTF-8 name.
    pub fn font_name_value(&self) -> Option<&str> {
        std::str::from_utf8(self.font_name_bytes()?).ok()
    }

    /// Decode a native UTF-8 or CESU-8 font name, borrowing when no conversion is needed.
    pub fn decoded_font_name_value(&self) -> Option<std::borrow::Cow<'_, str>> {
        cesu8::from_cesu8(self.font_name_bytes()?).ok()
    }

    fn font_name_bytes(&self) -> Option<&[u8]> {
        if self.kind != RichTextSpanType::FontName {
            return None;
        }
        let length = usize::from(u16::from_le_bytes(
            self.payload.get(8..10)?.try_into().ok()?,
        ));
        let name = self.payload.get(10..10 + length)?.strip_suffix(&[0])?;
        if name.contains(&0) {
            return None;
        }
        Some(name)
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

/// Decoded suggestion metadata from a rich-text span.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct RichTextSuggestion {
    /// Native suggestion category or flags, preserved without interpretation.
    pub suggestion_type: u32,
    /// Underline ARGB, before theme conversion.
    pub underline_argb: u32,
    /// Nonempty suggestions decoded from the stored UTF-16 list.
    pub strings: Vec<String>,
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

#[cfg(feature = "render")]
impl HyperlinkType {
    pub(crate) const fn is_hypertext(self) -> bool {
        !matches!(self, Self::Unknown | Self::Other(_))
    }
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
#[derive(Debug, Clone, PartialEq)]
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

/// Optional fields of the native page background. Unknown values stay intact.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub struct PageBackground {
    pub template_uri: Option<String>,
    pub image_id: Option<u32>,
    pub image_mode: Option<u32>,
    pub width: Option<u32>,
    pub rotation: Option<u32>,
    /// Saved field-8 records. `None` means absent; an empty list means present and empty.
    #[cfg_attr(feature = "serde", serde(default))]
    pub pdf_paper: Option<Vec<PdfPaperRecord>>,
    /// Saved field-9 template type, independent of PDF-record presence.
    #[cfg_attr(feature = "serde", serde(default))]
    pub template_type: Option<u32>,
}

/// One saved PDF page placement, before native binding checks or runtime scaling.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub struct PdfPaperRecord {
    /// Signed binding ID; negative sentinels are retained without resource admission.
    pub media_id: i32,
    /// Signed source PDF page index, independent of the physical note page.
    pub page_index: i32,
    /// Saved destination coordinates in left, top, right, bottom order.
    pub rectangle: PdfPaperRectangle,
}

/// Version-dependent encoding of a saved PDF destination rectangle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub enum PdfPaperRectangle {
    /// Signed integer coordinates in page formats 2034 and newer.
    Integer([i32; 4]),
    /// Exact IEEE-754 `f32` bits in older page formats, including nonfinite values.
    LegacyFloatBits([u32; 4]),
    /// Exact bytes when the fixed header does not identify the page format.
    Unspecified([u8; 16]),
}

/// Compatibility summary of page template metadata.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PageTemplate {
    /// Built-in template identifier, or the first PDF index for a custom-PDF summary.
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
    /// Compatibility summary of the first saved PDF record, not a native template-type assignment.
    CustomPdf {
        /// First source page index as unsigned bits; signed values remain in `PageBackground`.
        page_index: u32,
    },
}

/// A single pen stroke consisting of points and associated data.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Stroke {
    /// Saved pen settings, independent of the original sample arrays.
    #[cfg_attr(feature = "serde", serde(default))]
    pub rendering: Option<crate::StrokeRendering>,
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
        BulletType, DocumentMetadata, FormatVersion, HyperlinkType, LineSpacingType, ObjectType,
        ParagraphAlignment, ParagraphDirection, PredefinedTextStyle, RichTextParagraph,
        RichTextParagraphType, RichTextSpan, RichTextSpanType,
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
    fn color_span_argb_retains_alpha_and_rejects_truncated_payloads() {
        let mut span = RichTextSpan {
            kind: RichTextSpanType::BackgroundColor,
            start_utf16: 0,
            end_utf16: 1,
            interval_type: crate::SpanIntervalType::from(0),
            payload: Vec::new(),
        };
        for argb in [0x00345678_u32, 0x80345678, 0xff345678] {
            span.payload = argb.to_le_bytes().to_vec();
            assert_eq!(span.argb_value(), Some(argb));
            assert_eq!(
                span.color_value(),
                Some(super::Color {
                    r: 0x34,
                    g: 0x56,
                    b: 0x78
                })
            );
            span.payload.push(0xff);
            assert_eq!(span.argb_value(), Some(argb));
        }
        for length in 0..4 {
            span.payload = vec![0; length];
            assert_eq!(span.argb_value(), None);
            assert_eq!(span.color_value(), None);
        }
    }

    fn span_payload(kind: RichTextSpanType, payload: &[u8]) -> RichTextSpan {
        RichTextSpan {
            kind,
            start_utf16: 0,
            end_utf16: 1,
            interval_type: crate::SpanIntervalType::from(0),
            payload: payload.to_vec(),
        }
    }

    fn span_binary_capture() -> serde_json::Value {
        use sha2::Digest;

        const FIXTURE: &str = include_str!("../../../conformance/table-text-span-binary.json");
        assert_eq!(
            format!("{:x}", sha2::Sha256::digest(FIXTURE.as_bytes())),
            "0e8437fead4285c0ead18349f8708309219c74c6aae8a2acc83bec9fdbbc1c7b"
        );
        serde_json::from_str(FIXTURE).unwrap()
    }

    fn capture_bytes(value: &serde_json::Value) -> Vec<u8> {
        value
            .as_array()
            .unwrap()
            .iter()
            .map(|byte| u8::try_from(byte.as_u64().unwrap()).unwrap())
            .collect()
    }

    #[test]
    fn composition_and_suggestion_values_match_native_binary_reader_and_writer() {
        let capture = span_binary_capture();
        let mut readers = 0;
        let mut writers = 0;
        for case in capture["cases"].as_array().unwrap() {
            let kind = RichTextSpanType::from(case["kind"].as_u64().unwrap() as u32);
            if case["version"].as_u64() == Some(8)
                && matches!(
                    kind,
                    RichTextSpanType::ComposingBackgroundColor
                        | RichTextSpanType::Composing
                        | RichTextSpanType::ComposingTag
                        | RichTextSpanType::Suggestion
                )
            {
                let record = capture_bytes(&case["record"]);
                let available = case["available"].as_u64().unwrap() as usize;
                let span = span_payload(kind, record.get(16..available).unwrap_or_default());
                let original = span.payload.clone();
                let applied = case["applied"].as_bool().unwrap();
                match kind {
                    RichTextSpanType::ComposingBackgroundColor => assert_eq!(
                        span.composing_background_value(),
                        applied.then(|| case["decoded"]["color"].as_u64().unwrap() as u32),
                        "{}",
                        case["name"]
                    ),
                    RichTextSpanType::Composing | RichTextSpanType::ComposingTag => assert_eq!(
                        span.composition_value(),
                        applied.then(|| case["decoded"]["enabled"].as_bool().unwrap()),
                        "{}",
                        case["name"]
                    ),
                    RichTextSpanType::Suggestion => {
                        assert_native_suggestion(&span, case, applied);
                    }
                    _ => unreachable!(),
                }
                assert_eq!(span.payload, original);
                readers += 1;
            }
            if kind == RichTextSpanType::Suggestion && case["written"].as_bool() == Some(true) {
                let output = capture_bytes(&case["output"]);
                let span = span_payload(kind, &output[16..]);
                assert_native_suggestion(&span, case, true);
                writers += 1;
            }
        }
        assert_eq!(readers, 132);
        assert_eq!(writers, 60);
    }

    fn assert_native_suggestion(span: &RichTextSpan, case: &serde_json::Value, applied: bool) {
        let expected = applied.then(|| super::RichTextSuggestion {
            suggestion_type: case["decoded"]["suggestion_type"].as_u64().unwrap() as u32,
            underline_argb: case["decoded"]["underline"].as_u64().unwrap() as u32,
            strings: case["decoded"]["strings"]
                .as_array()
                .unwrap()
                .iter()
                .map(|text| text.as_str().unwrap().to_owned())
                .collect(),
        });
        assert_eq!(span.suggestion_value(), expected, "{}", case["name"]);
    }

    #[test]
    fn composition_requires_complete_modern_frames_and_matching_kinds() {
        for kind in [RichTextSpanType::Composing, RichTextSpanType::ComposingTag] {
            for length in 0..8 {
                assert_eq!(
                    span_payload(kind, &vec![0xff; length]).composition_value(),
                    None
                );
            }
            for value in [0, 1, 2, 0xff] {
                let mut payload = vec![0x7b; 12];
                payload[0] = value;
                assert_eq!(
                    span_payload(kind, &payload).composition_value(),
                    Some(value != 0)
                );
            }
        }
        for length in 0..8 {
            assert_eq!(
                span_payload(
                    RichTextSpanType::ComposingBackgroundColor,
                    &vec![0xff; length]
                )
                .composing_background_value(),
                None
            );
        }
        let unrelated = span_payload(RichTextSpanType::SpellCorrection, &[0; 16]);
        assert_eq!(unrelated.composition_value(), None);
        assert_eq!(unrelated.composing_background_value(), None);
        assert_eq!(unrelated.suggestion_value(), None);
    }

    #[test]
    fn suggestions_bound_declared_lists_and_reject_malformed_utf16() {
        let mut payload = vec![0; 12];
        for length in 0..12 {
            assert_eq!(
                span_payload(RichTextSpanType::Suggestion, &payload[..length]).suggestion_value(),
                None
            );
        }
        payload[8..12].copy_from_slice(&i32::MAX.to_le_bytes());
        assert_eq!(
            span_payload(RichTextSpanType::Suggestion, &payload).suggestion_value(),
            None
        );
        payload[8..12].copy_from_slice(&1_u32.to_le_bytes());
        payload.extend_from_slice(&u16::MAX.to_le_bytes());
        assert_eq!(
            span_payload(RichTextSpanType::Suggestion, &payload).suggestion_value(),
            None
        );
        payload[12..14].copy_from_slice(&1_u16.to_le_bytes());
        payload.extend_from_slice(&0xd800_u16.to_le_bytes());
        assert_eq!(
            span_payload(RichTextSpanType::Suggestion, &payload).suggestion_value(),
            None
        );
        for count in [i32::MIN, -1, 0] {
            payload[8..12].copy_from_slice(&count.to_le_bytes());
            assert_eq!(
                span_payload(RichTextSpanType::Suggestion, &payload)
                    .suggestion_value()
                    .unwrap()
                    .strings,
                Vec::<String>::new()
            );
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
    fn document_density_uses_native_default_dimensions_and_orientation() {
        let mut metadata = DocumentMetadata {
            default_page_dimensions: Some((1080, 1440)),
            page_dimensions: Some((360, 360)),
            flow_dimensions: Some((720, 720)),
            ..Default::default()
        };
        assert_eq!(metadata.document_density(), 3.0);
        metadata.orientation = Some(0);
        assert_eq!(metadata.document_density(), 3.0);
        for orientation in [1, 2, -1, i32::MIN] {
            metadata.orientation = Some(orientation);
            assert_eq!(metadata.document_density(), 4.0);
        }
        metadata.default_page_dimensions = Some((180, 720));
        metadata.orientation = Some(0);
        assert_eq!(metadata.document_density(), 0.5);
    }

    #[test]
    fn document_density_falls_back_for_absent_or_invalid_signed_axes() {
        let mut metadata = DocumentMetadata {
            page_dimensions: Some((1080, 1440)),
            flow_dimensions: Some((1080, 1440)),
            ..Default::default()
        };
        assert_eq!(metadata.document_density(), 1.0);
        for dimensions in [(0, 720), (u32::MAX, 720), (0x8000_0000, 720)] {
            metadata.default_page_dimensions = Some(dimensions);
            metadata.orientation = Some(0);
            assert_eq!(metadata.document_density(), 1.0);
            metadata.orientation = Some(1);
            assert_eq!(metadata.document_density(), 2.0);
        }
        for dimensions in [(720, 0), (720, u32::MAX), (720, 0x8000_0000)] {
            metadata.default_page_dimensions = Some(dimensions);
            metadata.orientation = Some(1);
            assert_eq!(metadata.document_density(), 1.0);
            metadata.orientation = Some(0);
            assert_eq!(metadata.document_density(), 2.0);
        }
    }

    fn font_name_span(payload: &[u8]) -> RichTextSpan {
        RichTextSpan {
            kind: RichTextSpanType::FontName,
            start_utf16: 0,
            end_utf16: 1,
            interval_type: crate::SpanIntervalType::from(0),
            payload: payload.to_vec(),
        }
    }

    #[test]
    fn font_names_use_native_byte_count_and_preserve_names_and_payloads() {
        let regular = font_name_span(&[
            0xde, 0xad, 0xbe, 0xef, 0x12, 0x34, 0x56, 0x78, 7, 0, b'R', b'o', b'b', b'o', b't',
            b'o', 0,
        ]);
        assert_eq!(regular.font_name_value(), Some("Roboto"));
        assert!(matches!(
            regular.decoded_font_name_value(),
            Some(std::borrow::Cow::Borrowed("Roboto"))
        ));

        let quoted_unicode = font_name_span(&[
            0xde, 0xad, 0xbe, 0xef, 0x12, 0x34, 0x56, 0x78, 8, 0, b' ', b'"', 0xe7, 0xad, 0x86,
            b'"', b' ', 0,
        ]);
        let original = quoted_unicode.payload.clone();
        assert_eq!(quoted_unicode.font_name_value(), Some(" \"筆\" "));
        assert_eq!(quoted_unicode.payload, original);
        let name = quoted_unicode.font_name_value().unwrap();
        assert_eq!(name.as_ptr(), quoted_unicode.payload[10..].as_ptr());

        let empty = font_name_span(&[0xde, 0xad, 0xbe, 0xef, 0x12, 0x34, 0x56, 0x78, 1, 0, 0]);
        assert_eq!(empty.font_name_value(), Some(""));
        assert_eq!(empty.decoded_font_name_value().as_deref(), Some(""));
    }

    #[test]
    fn font_names_decode_native_cesu8_surrogate_pairs_without_losing_payloads() {
        let span = font_name_span(&[
            0, 0, 0, 0, 0, 0, 0, 0, 13, 0, 0xe5, 0xad, 0x97, 0xe4, 0xbd, 0x93, 0xed, 0xa0, 0xb4,
            0xed, 0xb4, 0x9e, 0,
        ]);
        let original = span.payload.clone();
        assert_eq!(span.font_name_value(), None);
        assert!(
            matches!(span.decoded_font_name_value(), Some(std::borrow::Cow::Owned(ref name)) if name == "字体𝄞")
        );
        assert_eq!(span.payload, original);

        let mut utf8 = vec![0; 8];
        utf8.extend_from_slice(&11_u16.to_le_bytes());
        utf8.extend_from_slice("字体𝄞".as_bytes());
        utf8.push(0);
        let span = font_name_span(&utf8);
        assert!(matches!(
            span.decoded_font_name_value(),
            Some(std::borrow::Cow::Borrowed("字体𝄞"))
        ));
    }

    #[test]
    fn font_names_reject_truncated_and_malformed_native_payloads() {
        let valid = [
            0xde, 0xad, 0xbe, 0xef, 0x12, 0x34, 0x56, 0x78, 7, 0, b'R', b'o', b'b', b'o', b't',
            b'o', 0,
        ];
        for end in 0..valid.len() {
            assert_eq!(font_name_span(&valid[..end]).font_name_value(), None);
            assert_eq!(
                font_name_span(&valid[..end]).decoded_font_name_value(),
                None
            );
        }
        let mut wrong_kind = font_name_span(&valid);
        wrong_kind.kind = RichTextSpanType::ForegroundColor;
        assert_eq!(wrong_kind.font_name_value(), None);
        assert_eq!(wrong_kind.decoded_font_name_value(), None);

        let malformed: &[&[u8]] = &[
            &[0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
            &[0, 0, 0, 0, 0, 0, 0, 0, 0xff, 0xff, 0],
            &[0, 0, 0, 0, 0, 0, 0, 0, 2, 0, 0xff, 0],
            &[0, 0, 0, 0, 0, 0, 0, 0, 2, 0, b'A', b'B'],
            &[0, 0, 0, 0, 0, 0, 0, 0, 3, 0, b'A', 0, 0],
            &[0, 0, 0, 0, 0, 0, 0, 0, 3, 0, 0xe7, 0xad, 0],
            &[0, 0, 0, 0, 0, 0, 0, 0, 4, 0, 0xed, 0xa0, 0xb4, 0],
            &[0, 0, 0, 0, 0, 0, 0, 0, 4, 0, 0xed, 0xb4, 0x9e, 0],
            &[
                0, 0, 0, 0, 0, 0, 0, 0, 6, 0, 0xed, 0xa0, 0xb4, 0xed, 0xb4, 0,
            ],
            &[0, 0, 0, 0, 0, 0, 0, 0, 3, 0, 0xc0, 0x80, 0],
        ];
        for payload in malformed {
            assert_eq!(font_name_span(payload).font_name_value(), None);
            assert_eq!(font_name_span(payload).decoded_font_name_value(), None);
        }
    }

    #[test]
    fn font_names_match_native_binary_writer_outputs() {
        let capture = span_binary_capture();
        let mut count = 0;
        for case in capture["cases"].as_array().unwrap() {
            if case["kind"].as_u64() != Some(4) || case["written"].as_bool() != Some(true) {
                continue;
            }
            let bytes = capture_bytes(&case["output"]);
            let span = font_name_span(&bytes[16..]);
            let expected = case["decoded"]["name"].as_str().unwrap();
            assert_eq!(
                span.decoded_font_name_value().as_deref(),
                Some(expected),
                "{}",
                case["name"]
            );
            for end in 0..span.payload.len() {
                assert_eq!(
                    font_name_span(&span.payload[..end]).decoded_font_name_value(),
                    None,
                    "{} truncated at {end}",
                    case["name"]
                );
            }
            count += 1;
        }
        assert_eq!(count, 60);
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
            interval_type: crate::SpanIntervalType::from(0),
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

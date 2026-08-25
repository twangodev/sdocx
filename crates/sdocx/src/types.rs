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
    /// A placed image object.
    Image {
        /// Placement box in page coordinates.
        bbox: BoundingBox,
        /// Index into `DocumentMetadata::media_assets`.
        media_index: usize,
    },
    /// A rich text object.
    TextBox(RichTextBox),
}

impl PageElement {
    /// Return the S Pen SDK object type represented by this element.
    pub const fn object_type(&self) -> ObjectType {
        match self {
            Self::Image { .. } => ObjectType::Image,
            Self::TextBox(_) => ObjectType::TextBox,
        }
    }
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
    /// Text margins in left, top, right, bottom order.
    pub margins: Option<[f32; 4]>,
    /// Raw Android text-gravity flags.
    pub gravity: Option<u8>,
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
    /// Start offset in UTF-16 code units, inclusive.
    pub start_utf16: u32,
    /// End offset in UTF-16 code units, exclusive.
    pub end_utf16: u32,
    /// Type-specific payload retained for forward compatibility.
    pub payload: Vec<u8>,
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
            raw => Self::Other(raw),
        }
    }
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
    use super::{FormatVersion, ObjectType};

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
}

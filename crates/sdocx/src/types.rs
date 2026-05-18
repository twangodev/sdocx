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
    /// MIME type, when recognized.
    pub mime_type: String,
    /// Raw media bytes.
    pub data: Vec<u8>,
}

/// A non-stroke page element.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
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
    /// Pressure values for each point, normalized to `[0.0, 1.0]`.
    pub pressures: Vec<f64>,
    /// Timestamps for each point in milliseconds since the Unix epoch.
    pub timestamps: Vec<i64>,
    /// Stylus tilt along the X axis for each point.
    pub tilt_x: Vec<i64>,
    /// Stylus tilt along the Y axis for each point.
    pub tilt_y: Vec<i64>,
    /// Stroke color, if present.
    pub color: Option<Color>,
    /// Pen width in pixels.
    pub pen_width: f32,
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

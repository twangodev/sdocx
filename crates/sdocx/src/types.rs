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
    /// Default page dimensions as `(width, height)` in pixels.
    pub page_dimensions: Option<(u32, u32)>,
    /// Ordered list of page UUIDs.
    pub page_ids: Vec<String>,
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
    /// The strokes drawn on this page.
    pub strokes: Vec<Stroke>,
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

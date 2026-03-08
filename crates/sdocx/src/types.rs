#[derive(Debug, Clone)]
pub struct Document {
    pub pages: Vec<Page>,
    pub metadata: DocumentMetadata,
}

#[derive(Debug, Clone, Default)]
pub struct DocumentMetadata {
    pub created_ms: Option<i64>,
    pub modified_ms: Option<i64>,
    pub background_color: Option<Color>,
    pub page_dimensions: Option<(u32, u32)>,
    pub page_ids: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct Page {
    pub uuid: String,
    pub width: u32,
    pub height: u32,
    pub content_bbox: BoundingBox,
    pub strokes: Vec<Stroke>,
}

#[derive(Debug, Clone)]
pub struct Stroke {
    pub bbox: BoundingBox,
    pub points: Vec<Point>,
    pub pressures: Vec<f64>,
    pub timestamps: Vec<i64>,
    pub tilt_x: Vec<i64>,
    pub tilt_y: Vec<i64>,
    pub color: Option<Color>,
    pub pen_width: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Point {
    pub x: f64,
    pub y: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BoundingBox {
    pub x_min: f64,
    pub y_min: f64,
    pub x_max: f64,
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

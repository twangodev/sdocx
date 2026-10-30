//! Explicit WaterColor destination-resolution canvas using captured host arithmetic.

use crate::ink::watercolor;
use crate::{RenderedPage, Stroke};
use base64::Engine as _;

const MAX_PIXELS: usize = 8 * 1024 * 1024;
const MAX_WORK: usize = 64 * 1024 * 1024;
const MAX_SUBMISSIONS: usize = 256;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum InkRasterOrigin {
    TopLeft,
    BottomLeft,
}

/// Explicit source-coordinate projection into the destination pixel frame.
/// Only the captured uniform output scales 1 and 2 are currently admitted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct InkRasterContext {
    pub width: u32,
    pub height: u32,
    pub pixels_per_unit: u32,
    pub origin: InkRasterOrigin,
}

impl InkRasterContext {
    /// Validate the context and return the bounded destination pixel count.
    pub fn pixel_count(self) -> Result<usize, InkRasterError> {
        pixel_count(self)
    }
}

/// Selecting this profile opts into the captured host behavior.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub enum InkRasterProfile {
    /// GLV1/null or GLV2/empty settings, glibc-FMA sampling and captured Mesa
    /// RTV1 paint. Fresh private mask per submission, native redraw enabled.
    /// Source-screen dimensions are independent of the output frame.
    WaterColorHostRtv1 {
        screen_width: u32,
        screen_height: u32,
    },
}

#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum InkRasterError {
    #[error("invalid ink raster frame or projection")]
    InvalidContext,
    #[error("backdrop must contain exactly one premultiplied RGBA8 pixel per frame pixel")]
    InvalidBackdrop,
    #[error("unsupported native ink input: {0}")]
    UnsupportedStroke(&'static str),
    #[error("invalid native ink input: {0}")]
    InvalidStroke(&'static str),
    #[error("native ink raster work limit exceeded")]
    WorkLimit,
    #[error("native ink raster allocation failed")]
    Allocation,
    #[error("native ink raster PNG encoding failed")]
    Encoding,
    #[cfg(feature = "pdf")]
    #[error("native ink raster PDF export failed: {0}")]
    Pdf(#[from] crate::PdfError),
}

/// Destination pixels in top-origin premultiplied RGBA8 order.
pub struct InkRasterCanvas {
    context: InkRasterContext,
    pixels: Vec<[u8; 4]>,
    remaining_work: usize,
    remaining_submissions: usize,
}

impl InkRasterCanvas {
    pub fn new(context: InkRasterContext, clear: [u8; 4]) -> Result<Self, InkRasterError> {
        let count = pixel_count(context)?;
        if !premultiplied(clear) {
            return Err(InkRasterError::InvalidBackdrop);
        }
        let mut pixels = Vec::new();
        pixels
            .try_reserve_exact(count)
            .map_err(|_| InkRasterError::Allocation)?;
        pixels.resize(count, clear);
        Self::from_pixels(context, pixels)
    }

    /// Pixels use top-origin row order and premultiplied RGBA8 channels.
    pub fn from_pixels(
        context: InkRasterContext,
        pixels: Vec<[u8; 4]>,
    ) -> Result<Self, InkRasterError> {
        let count = pixel_count(context)?;
        if pixels.len() != count || pixels.iter().any(|&pixel| !premultiplied(pixel)) {
            return Err(InkRasterError::InvalidBackdrop);
        }
        Ok(Self {
            context,
            pixels,
            remaining_work: MAX_WORK,
            remaining_submissions: MAX_SUBMISSIONS,
        })
    }

    pub fn context(&self) -> InkRasterContext {
        self.context
    }

    pub fn pixels(&self) -> &[[u8; 4]] {
        &self.pixels
    }

    /// Paint atomically: failed source validation or exhausted work preserves pixels.
    pub fn paint(
        &mut self,
        stroke: &Stroke,
        profile: InkRasterProfile,
    ) -> Result<(), InkRasterError> {
        if self.remaining_submissions == 0 {
            return Err(InkRasterError::WorkLimit);
        }
        let InkRasterProfile::WaterColorHostRtv1 {
            screen_width,
            screen_height,
        } = profile;
        let (rows, argb) = watercolor::prepare_stroke(stroke, [screen_width, screen_height])?;
        let mut matrix = [0.; 16];
        let scale = self.context.pixels_per_unit as f32;
        matrix[0] = 2. * scale / self.context.width as f32;
        matrix[5] = 2. * scale / self.context.height as f32;
        matrix[12] = -1.;
        matrix[13] = -1.;
        matrix[15] = 1.;
        if self.context.origin == InkRasterOrigin::TopLeft {
            matrix[5] = -matrix[5];
            matrix[13] = 1.;
        }
        let plan = watercolor::raster::prepare(
            &rows,
            [self.context.width, self.context.height],
            matrix,
            argb,
            self.remaining_work,
        )?;
        let pixels = plan.render(&self.pixels)?;
        self.remaining_work -= plan.work();
        self.remaining_submissions -= 1;
        self.pixels = pixels;
        Ok(())
    }
    /// Encode straight-alpha PNG for display at the destination's intrinsic size.
    pub fn to_png(&self) -> Result<Vec<u8>, InkRasterError> {
        let rgba: Vec<_> = self
            .pixels
            .iter()
            .flat_map(|&[r, g, b, a]| {
                let straight = |v: u8| {
                    if a == 0 {
                        0
                    } else {
                        ((u32::from(v) * 255 + u32::from(a) / 2) / u32::from(a)).min(255) as u8
                    }
                };
                [straight(r), straight(g), straight(b), a]
            })
            .collect();
        let mut bytes = Vec::new();
        {
            let mut encoder =
                png::Encoder::new(&mut bytes, self.context.width, self.context.height);
            encoder.set_color(png::ColorType::Rgba);
            encoder.set_depth(png::BitDepth::Eight);
            encoder
                .write_header()
                .map_err(|_| InkRasterError::Encoding)?
                .write_image_data(&rgba)
                .map_err(|_| InkRasterError::Encoding)?;
        }
        Ok(bytes)
    }

    /// Export the resolved pixel frame through the retained-scene PDF path.
    /// Canvas pixels define the intrinsic page size; PdfOptions controls DPI.
    #[cfg(feature = "pdf")]
    pub fn to_pdf(&self, options: &crate::PdfOptions) -> Result<Vec<u8>, InkRasterError> {
        let png = self.to_png()?;
        let scene =
            crate::render::raster_canvas_scene(self.context.width, self.context.height, &png);
        Ok(crate::pdf::render_scenes_pdf_with_progress(
            &[scene],
            options,
            &mut |_| {},
        )?)
    }

    /// Image-backed SVG accepted by the public SVG preview and SVG-page PDF API.
    /// The page is a resolved pixel frame; it does not retain editable vectors.
    pub fn rendered_page(&self, source_page_index: usize) -> Result<RenderedPage, InkRasterError> {
        let png = self.to_png()?;
        let data = base64::engine::general_purpose::STANDARD.encode(png);
        let width = self.context.width;
        let height = self.context.height;
        let svg = format!(
            "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{width}\" height=\"{height}\" viewBox=\"0 0 {width} {height}\"><image width=\"{width}\" height=\"{height}\" image-rendering=\"optimizeSpeed\" style=\"image-rendering:pixelated\" href=\"data:image/png;base64,{data}\"/></svg>"
        );
        Ok(RenderedPage::new(source_page_index, width, height, svg))
    }
}

fn premultiplied([r, g, b, a]: [u8; 4]) -> bool {
    r <= a && g <= a && b <= a
}

fn pixel_count(context: InkRasterContext) -> Result<usize, InkRasterError> {
    let count = (context.width as usize)
        .checked_mul(context.height as usize)
        .ok_or(InkRasterError::InvalidContext)?;
    if count == 0
        || count > MAX_PIXELS
        || context.width > 8192
        || context.height > 8192
        || !matches!(context.pixels_per_unit, 1 | 2)
    {
        return Err(InkRasterError::InvalidContext);
    }
    Ok(count)
}

//! Explicit destination-resolution ink canvas using captured host arithmetic.

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
    /// Captured native Drawing RTV4 paint with enhanced AA disabled.
    /// Each stamp blends directly; the native shader uses RGB and ignores
    /// the supplied color's alpha. Context and color remain explicit inputs.
    FountainPenV14DirectHost {
        source_origin: [i32; 2],
        paint_argb: u32,
    },
    /// Fresh enhanced-AA/redraw RTV4 paint from normal saved Fountain V14.
    /// Integer page-coordinate origin for a cropped output and explicit native
    /// RT paint ARGB; this color is a caller assertion, independent of geometry.
    /// Native normalized XY clipping supports quads crossing the output frame.
    FountainPenV14Host {
        source_origin: [i32; 2],
        paint_argb: u32,
    },

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

/// Caller-supplied texture in GL upload row order. Only its red channel is sampled.
#[derive(Clone, Copy, Debug)]
pub struct BrushPenTexture<'a> {
    pub width: usize,
    pub height: usize,
    pub rgba: &'a [u8],
}

/// Explicit native V1 uniforms for the conditional host-Mesa paint model.
/// The projection maps stroke coordinates to clip coordinates. Grain parameters
/// are in bottom-origin framebuffer pixels; textures use GL upload row order.
/// This projection supersedes the canvas origin and pixels-per-unit settings.
/// `rgb` supplies normalized color; serialized stroke color alpha is ignored by
/// this captured shader. Stamp pressure and texture coverage determine alpha.
#[derive(Clone, Copy, Debug)]
pub struct BrushPenPaintContext<'a> {
    pub screen_width: i32,
    pub screen_height: i32,
    pub projection: [f32; 16],
    pub rgb: [f32; 3],
    pub grain_period: [f32; 2],
    pub grain_offset: [f32; 2],
    pub num_tiles: f32,
    pub atlas: BrushPenTexture<'a>,
    pub grain: BrushPenTexture<'a>,
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
        let fountain = match profile {
            InkRasterProfile::FountainPenV14Host {
                source_origin,
                paint_argb,
            } => Some((source_origin, paint_argb, false)),
            InkRasterProfile::FountainPenV14DirectHost {
                source_origin,
                paint_argb,
            } => Some((source_origin, paint_argb, true)),
            _ => None,
        };
        if let Some((source_origin, paint_argb, direct)) = fountain {
            let plan = crate::ink::fountain_v14::raster::prepare(
                stroke,
                self.context,
                source_origin,
                paint_argb,
                direct,
                self.remaining_work,
            )?;
            let pixels = plan.render(&self.pixels)?;
            self.remaining_work -= plan.work();
            self.remaining_submissions -= 1;
            self.pixels = pixels;
            return Ok(());
        }
        let InkRasterProfile::WaterColorHostRtv1 {
            screen_width,
            screen_height,
        } = profile else { unreachable!("Fountain profile returned above") };
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
    /// Paint a sampled GLV10 stroke using caller-supplied native textures and
    /// uniforms. This conditional Mesa host model does not infer device context.
    /// Invalid inputs and exhausted work leave the canvas unchanged.
    /// Sampling uses its own one-million-sample limit before the canvas checks
    /// its remaining paint budget; that budget does not bound sampler work.
    pub fn paint_brushpen_glv10_host(
        &mut self,
        stroke: &Stroke,
        context: BrushPenPaintContext<'_>,
    ) -> Result<(), InkRasterError> {
        use crate::ink::brushpen::paint::{self, Context, Error, Rgba8};
        if self.remaining_submissions == 0 {
            return Err(InkRasterError::WorkLimit);
        }
        let records =
            crate::sample_brushpen_glv10_host(stroke, context.screen_width, context.screen_height)
                .ok_or(InkRasterError::UnsupportedStroke(
                    "unsupported BrushPen GLV10 source or display",
                ))?;
        // A full-frame scan per record is a conservative upper bound on paint work.
        let work = records
            .len()
            .checked_mul(self.pixels.len())
            .and_then(|n| n.checked_add(self.pixels.len()))
            .ok_or(InkRasterError::WorkLimit)?;
        let remaining = self
            .remaining_work
            .checked_sub(work)
            .ok_or(InkRasterError::WorkLimit)?;
        let mut bytes = Vec::new();
        bytes
            .try_reserve_exact(self.pixels.len() * 4)
            .map_err(|_| InkRasterError::Allocation)?;
        for row in self.pixels.chunks_exact(self.context.width as usize).rev() {
            for pixel in row {
                bytes.extend_from_slice(pixel);
            }
        }
        fn texture(t: BrushPenTexture<'_>) -> Rgba8<'_> {
            Rgba8 {
                width: t.width,
                height: t.height,
                bytes: t.rgba,
            }
        }
        paint::paint(
            &records,
            texture(context.atlas),
            texture(context.grain),
            Context {
                width: self.context.width as usize,
                height: self.context.height as usize,
                projection: context.projection,
                rgb: context.rgb,
                grain_period: context.grain_period,
                grain_offset: context.grain_offset,
                num_tiles: context.num_tiles,
            },
            &mut bytes,
        )
        .map_err(|error| match error {
            Error::Texture | Error::Context => InkRasterError::InvalidContext,
            Error::Record => InkRasterError::InvalidStroke("unrepresentable BrushPen stamp"),
        })?;
        for (row, source) in self
            .pixels
            .chunks_exact_mut(self.context.width as usize)
            .rev()
            .zip(bytes.chunks_exact(self.context.width as usize * 4))
        {
            for (pixel, rgba) in row.iter_mut().zip(source.as_chunks::<4>().0) {
                pixel.copy_from_slice(rgba);
            }
        }
        self.remaining_work = remaining;
        self.remaining_submissions -= 1;
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

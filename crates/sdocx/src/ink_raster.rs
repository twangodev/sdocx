//! Explicit destination-resolution ink canvas using captured host arithmetic.

pub use crate::ink::pencil2::offset_candidate::{
    Pencil2V14HostGrainState, pencil2_v14_host_grain_offset,
};
use crate::ink::{colored_pencil, pencil2, pencil2::device_raster, watercolor};
use crate::{RenderedPage, Stroke};
use base64::Engine as _;

pub use crate::ink::fountain_v14::raster::{FountainV14Prepared, FountainV14Scene};
mod fountain_page;

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

/// Native depth bitmap parameters for the explicit ColoredPencil host profile.
/// RGBA rows use native upload order: row zero is sampled at low V near the
/// canvas bottom. Only red is sampled; bitmap alpha is independent of coverage.
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct InkRasterDepthContext {
    pub width: u32,
    pub height: u32,
    pub background_scale: f32,
}

impl InkRasterDepthContext {
    /// Reconstruct GLV4's native background scale from explicit system-display
    /// dimensions. Output page dimensions do not establish the display context.
    pub fn from_colored_pencil_display(
        width: u32,
        height: u32,
        display_width: u32,
        display_height: u32,
    ) -> Result<Self, InkRasterError> {
        if !(1..=16_384).contains(&display_width) || !(1..=16_384).contains(&display_height) {
            return Err(InkRasterError::InvalidContext);
        }
        let context = Self {
            width,
            height,
            background_scale: display_width.min(display_height) as f32 / 1440.,
        };
        context.pixel_count()?;
        Ok(context)
    }

    pub fn pixel_count(self) -> Result<usize, InkRasterError> {
        if !(1..=8192).contains(&self.width)
            || !(1..=8192).contains(&self.height)
            || !self.background_scale.is_finite()
            || !(1. / 1440. ..=16_384. / 1440.).contains(&self.background_scale)
        {
            return Err(InkRasterError::InvalidDepthMap);
        }
        let count = self.width as usize * self.height as usize;
        if count > MAX_PIXELS {
            return Err(InkRasterError::WorkLimit);
        }
        Ok(count)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum InkRasterInverseScale {
    One,
    Half,
}

/// Selecting this profile opts into the captured host behavior.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub enum InkRasterProfile {
    /// Fresh source-0 GLV14 host paint with explicit geometry inverse scale.
    Pencil2V14 {
        inverse_scale: InkRasterInverseScale,
    },
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
    /// Fresh GLV4 redraw with literal `4;`, glibc TYPE_3 RNG and portable libm,
    /// selected PenData spacing-extra zero, and RTV1. Ordinary paint has no
    /// depth map; paint_with_depth_map supplies the optional native bitmap.
    /// Native selected size clamps to 0.4..=800; explicit system-display
    /// dimensions must each be in 1..=16384 and are independent of page size.
    ColoredPencilV4Host {
        pen_data_width: u32,
        pen_data_height: u32,
    },
}

#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum InkRasterError {
    #[error("invalid ink raster frame or projection")]
    InvalidContext,
    #[error("backdrop must contain exactly one premultiplied RGBA8 pixel per frame pixel")]
    InvalidBackdrop,
    #[error("invalid native depth bitmap or background scale")]
    InvalidDepthMap,
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
        if let InkRasterProfile::ColoredPencilV4Host {
            pen_data_width,
            pen_data_height,
        } = profile
        {
            return self.paint_colored_pencil(stroke, [pen_data_width, pen_data_height], None);
        }
        if let InkRasterProfile::Pencil2V14 { inverse_scale } = profile {
            return self.paint_pencil2_with_grain_offset(stroke, inverse_scale, [0., 0.], [0., 0.]);
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
        } = profile
        else {
            unreachable!("Fountain profile returned above")
        };
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
    /// Paint a caller-owned direct Fountain snapshot over the current pixels.
    /// The canvas scale must match the scale used to create the snapshot.
    /// `source_origin` is the integer source-space crop origin; the context
    /// chooses the output row orientation. The direct host shader ignores the
    /// alpha component of `paint_argb`. Unsupported inputs leave pixels and
    /// budgets unchanged.
    pub fn paint_fountain_v14_prepared(
        &mut self,
        snapshot: &FountainV14Prepared,
        source_origin: [i32; 2],
        paint_argb: u32,
    ) -> Result<(), InkRasterError> {
        if self.remaining_submissions == 0 {
            return Err(InkRasterError::WorkLimit);
        }
        let plan = crate::ink::fountain_v14::raster::prepare_snapshot(
            snapshot,
            self.context,
            source_origin,
            paint_argb,
            self.remaining_work,
        )?;
        let pixels = plan.render(&self.pixels)?;
        self.remaining_work -= plan.work();
        self.remaining_submissions -= 1;
        self.pixels = pixels;
        Ok(())
    }

    /// Paint Pencil2 with a translation in framebuffer pixels, with positive X
    /// right and positive Y up, regardless of the source coordinate origin.
    /// Texture grain remains fixed to framebuffer coordinates. Geometry inverse
    /// scale is independent of this translation and must be supplied by the caller.
    /// This is a bounded host profile, not a general native drawing transform.
    /// Source sampling has separate 16,384-point and 16,384-record limits;
    /// it runs before the canvas paint budget and is not charged to that budget.
    pub fn paint_pencil2_translated(
        &mut self,
        stroke: &Stroke,
        inverse_scale: InkRasterInverseScale,
        translation: [f32; 2],
    ) -> Result<(), InkRasterError> {
        if translation
            .iter()
            .any(|v| !v.is_finite() || v.abs() > 16_384.)
        {
            return Err(InkRasterError::InvalidContext);
        }
        self.paint_with_pencil_translation(
            stroke,
            InkRasterProfile::Pencil2V14 { inverse_scale },
            translation,
            [0., 0.],
        )
    }

    /// Paint one Pencil2 submission with an explicitly captured native grain offset.
    ///
    /// Both translation and grain offset are in framebuffer pixels, with X right
    /// and Y up regardless of source origin. The grain offset is subtracted from
    /// framebuffer pixel centers before sampling; it does not move geometry. The
    /// caller must supply the captured f32 values without rounding. This applies
    /// only to this submission and does not infer any native model transform.
    /// Offsets are bounded to +/-16384 pixels as a candidate resource/precision
    /// guard, not a native limit. Other paint methods use zero grain offset.
    /// Sampling has separate 16,384-point and 16,384-record limits and is not
    /// charged to the canvas paint budget.
    /// Unsupported or invalid inputs leave pixels and budgets unchanged.
    pub fn paint_pencil2_with_grain_offset(
        &mut self,
        stroke: &Stroke,
        inverse_scale: InkRasterInverseScale,
        translation: [f32; 2],
        grain_offset: [f32; 2],
    ) -> Result<(), InkRasterError> {
        self.paint_with_pencil_translation(
            stroke,
            InkRasterProfile::Pencil2V14 { inverse_scale },
            translation,
            grain_offset,
        )
    }

    fn paint_with_pencil_translation(
        &mut self,
        stroke: &Stroke,
        profile: InkRasterProfile,
        translation: [f32; 2],
        grain_offset: [f32; 2],
    ) -> Result<(), InkRasterError> {
        if translation
            .iter()
            .chain(grain_offset.iter())
            .any(|v| !v.is_finite() || v.abs() > 16_384.)
        {
            return Err(InkRasterError::InvalidContext);
        }
        if self.remaining_submissions == 0 {
            return Err(InkRasterError::WorkLimit);
        }
        let inverse_scale = match profile {
            InkRasterProfile::ColoredPencilV4Host { .. }
            | InkRasterProfile::WaterColorHostRtv1 { .. }
            | InkRasterProfile::FountainPenV14Host { .. }
            | InkRasterProfile::FountainPenV14DirectHost { .. } => {
                unreachable!("family raster branch returned above")
            }
            InkRasterProfile::Pencil2V14 {
                inverse_scale: InkRasterInverseScale::One,
            } => 1.,
            InkRasterProfile::Pencil2V14 {
                inverse_scale: InkRasterInverseScale::Half,
            } => 0.5,
        };
        let sampled =
            pencil2::prepare_stroke(stroke, inverse_scale).map_err(|error| match error {
                pencil2::SamplerError::Unsupported(message) => {
                    InkRasterError::UnsupportedStroke(message)
                }
                pencil2::SamplerError::Invalid(message) => InkRasterError::InvalidStroke(message),
                pencil2::SamplerError::WorkLimit => InkRasterError::WorkLimit,
                #[cfg(all(test, feature = "serde"))]
                pencil2::SamplerError::MissingNativeState(message) => {
                    InkRasterError::UnsupportedStroke(message)
                }
            })?;
        let records: Vec<_> = sampled.into_iter().map(|record| record.lanes).collect();
        let mut context = device_context(self.context);
        context.translation = translation;
        context.grain_offset = grain_offset;
        let plan = device_raster::prepare(&records, context).map_err(|error| match error {
            device_raster::RasterError::Context => InkRasterError::InvalidContext,
            device_raster::RasterError::Record => {
                InkRasterError::InvalidStroke("unrepresentable derived stamp")
            }
            device_raster::RasterError::WorkLimit => InkRasterError::WorkLimit,
            device_raster::RasterError::Allocation => InkRasterError::Allocation,
        })?;
        let work = plan.work();
        let remaining = self
            .remaining_work
            .checked_sub(work)
            .ok_or(InkRasterError::WorkLimit)?;
        let pixels = plan
            .render(Some(&self.pixels))
            .ok_or(InkRasterError::Allocation)?;
        self.pixels = pixels;
        self.remaining_work = remaining;
        self.remaining_submissions -= 1;
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

    /// Paint with the supplied native depth bitmap, atomically. Currently this
    /// requires ColoredPencilV4Host; depth pixels need not be premultiplied.
    /// The map is borrowed for this submission and is not retained by the canvas.
    pub fn paint_with_depth_map(
        &mut self,
        stroke: &Stroke,
        profile: InkRasterProfile,
        depth: InkRasterDepthContext,
        depth_pixels: &[[u8; 4]],
    ) -> Result<(), InkRasterError> {
        let InkRasterProfile::ColoredPencilV4Host {
            pen_data_width,
            pen_data_height,
        } = profile
        else {
            return Err(InkRasterError::UnsupportedStroke(
                "depth map requires ColoredPencilV4Host",
            ));
        };
        if depth.pixel_count()? != depth_pixels.len() {
            return Err(InkRasterError::InvalidDepthMap);
        }
        self.paint_colored_pencil(
            stroke,
            [pen_data_width, pen_data_height],
            Some(colored_pencil::raster::DepthMap {
                width: depth.width as usize,
                height: depth.height as usize,
                background_scale: depth.background_scale,
                pixels: depth_pixels,
            }),
        )
    }

    fn paint_colored_pencil(
        &mut self,
        stroke: &Stroke,
        pen_data: [u32; 2],
        depth: Option<colored_pencil::raster::DepthMap<'_>>,
    ) -> Result<(), InkRasterError> {
        if self.remaining_submissions == 0 {
            return Err(InkRasterError::WorkLimit);
        }
        let (rows, argb) = colored_pencil::prepare_stroke(stroke, pen_data)?;
        let plan = if let Some(depth) = depth {
            colored_pencil::raster::prepare_depth(
                &rows,
                self.context,
                argb,
                self.remaining_work,
                Some(depth),
            )?
        } else {
            colored_pencil::raster::prepare(&rows, self.context, argb, self.remaining_work)?
        };
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

fn device_context(context: InkRasterContext) -> device_raster::Context {
    device_raster::Context {
        frame: [context.width, context.height],
        scale: context.pixels_per_unit as f32,
        translation: [0., 0.],
        grain_offset: [0., 0.],
        y_up: context.origin == InkRasterOrigin::BottomLeft,
    }
}

#[cfg(all(test, feature = "serde"))]
mod fountain_snapshot_budget_tests {
    use super::*;

    #[test]
    fn page_dispatch_rejection_and_paint_failure_scene_contract() {
        let inputs: serde_json::Value = serde_json::from_str(include_str!(
            "../tests/assets/fountain-v14/scene-sources.json"
        ))
        .unwrap();
        let stroke: Stroke =
            serde_json::from_value(inputs["strokes"][0]["stroke"].clone()).unwrap();
        let page = crate::Page {
            uuid: String::new(),
            width: 8,
            height: 8,
            content_bbox: Default::default(),
            background_color: None,
            template: None,
            background: Default::default(),
            objects: vec![stroke.clone().into()],
        };
        let context = InkRasterContext {
            width: 8,
            height: 8,
            pixels_per_unit: 1,
            origin: InkRasterOrigin::TopLeft,
        };
        let mut canvas = InkRasterCanvas::new(context, [0; 4]).unwrap();
        let mut scene = FountainV14Scene::default();
        let mut unsupported = page.clone();
        if let crate::PageObjectContent::Stroke(stroke) = &mut unsupported.objects[0].content {
            stroke.points.clear();
        }
        assert!(
            canvas
                .paint_fountain_v14_page_stroke(
                    &mut scene,
                    &unsupported,
                    0,
                    9,
                    0,
                    [0, 0],
                    0xff000000
                )
                .is_err()
        );
        assert!(
            canvas
                .paint_fountain_v14_page_stroke(&mut scene, &page, 1, 9, 0, [0, 0], 0xff000000)
                .is_err()
        );
        let mut control = FountainV14Scene::default();
        let expected = control
            .prepare(9, 0, &stroke, context)
            .unwrap()
            .outgoing_direction();
        let actual = scene
            .prepare(9, 0, &stroke, context)
            .unwrap()
            .outgoing_direction();
        assert_eq!(actual, expected);
        scene.reset();
        canvas.remaining_submissions = 0;
        assert!(
            canvas
                .paint_fountain_v14_page_stroke(&mut scene, &page, 0, 9, 0, [0, 0], 0xff000000)
                .is_err()
        );
        assert!(canvas.pixels().iter().all(|pixel| *pixel == [0; 4]));
        // A same-key cache hit retains the original snapshot despite this
        // deliberately changed source, demonstrating the documented commit.
        let mut changed = stroke.clone();
        changed.points.clear();
        assert_eq!(
            scene
                .prepare(9, 0, &changed, context)
                .unwrap()
                .outgoing_direction(),
            expected
        );
    }

    #[test]
    fn rejected_prepared_paint_preserves_work_and_pixels() {
        let inputs: serde_json::Value = serde_json::from_str(include_str!(
            "../tests/assets/fountain-v14/scene-sources.json"
        ))
        .unwrap();
        let stroke: Stroke =
            serde_json::from_value(inputs["strokes"][0]["stroke"].clone()).unwrap();
        let context = InkRasterContext {
            width: 8,
            height: 8,
            pixels_per_unit: 1,
            origin: InkRasterOrigin::TopLeft,
        };
        let prepared =
            FountainV14Prepared::new(&stroke, context, crate::Point { x: 0., y: 1. }).unwrap();
        let mut canvas = InkRasterCanvas::new(context, [0; 4]).unwrap();
        canvas.remaining_work = 0;
        let before = canvas.pixels.clone();
        let submissions = canvas.remaining_submissions;
        assert!(matches!(
            canvas.paint_fountain_v14_prepared(&prepared, [0, 0], 0xff000000),
            Err(InkRasterError::WorkLimit)
        ));
        assert_eq!(canvas.pixels, before);
        assert_eq!(canvas.remaining_work, 0);
        assert_eq!(canvas.remaining_submissions, submissions);
    }
}

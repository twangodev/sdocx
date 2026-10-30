//! Fixed-resolution host profile used by the explicit ink raster canvas.
//! Source fragment channels and each destination blend are quantized separately.
//! Normal document dispatch and uncaptured Drawing contexts remain unverified.

const GRAIN: &[u8] = include_bytes!("assets/grain.r8");
const POINT: &[u8] = include_bytes!("assets/point.r8");
pub(crate) const MAX_PIXELS: usize = 8 * 1024 * 1024;
const MAX_SAMPLES: usize = 16 * 1024 * 1024;
const MAX_RECORDS: usize = 16_384;

#[derive(Clone, Copy)]
pub(crate) struct Context {
    pub frame: [u32; 2],
    pub scale: f32,
    pub translation: [f32; 2],
    pub grain_offset: [f32; 2],
    pub y_up: bool,
}

impl Default for Context {
    fn default() -> Self {
        Self {
            frame: [192, 192],
            scale: 1.,
            translation: [0., 0.],
            grain_offset: [0., 0.],
            y_up: true,
        }
    }
}

#[cfg(test)]
pub(crate) fn records(bytes: &[u8]) -> Vec<[f32; 14]> {
    assert!(bytes.len().is_multiple_of(56));
    bytes
        .as_chunks::<56>()
        .0
        .iter()
        .map(|row| {
            std::array::from_fn(|i| f32::from_le_bytes(row[i * 4..i * 4 + 4].try_into().unwrap()))
        })
        .collect()
}

pub(crate) fn over(destination: &mut [u8; 4], source: [u8; 4]) {
    *destination = crate::ink::host_rgba8::over(*destination, source);
}

fn sample(tex: &[u8], size: [usize; 2], uv: [f32; 2], clamp: bool) -> u8 {
    let axis = |q: f32, n: usize| {
        let q = if clamp { q } else { q - q.floor() };
        let i = (q * n as f32).floor() as i32;
        if clamp {
            i.clamp(0, n as i32 - 1) as usize
        } else {
            i.rem_euclid(n as i32) as usize
        }
    };
    tex[axis(uv[1], size[1]) * size[0] + axis(uv[0], size[0])]
}

fn fragment(r: &[f32; 14], uv: [f32; 3], pixel: [f32; 2], context: Context) -> [u8; 4] {
    let [fx, negative_w, point_v] = uv;
    if sample(POINT, [100, 300], [fx, point_v], true) == 0 {
        return [0; 4];
    }
    let period = 75. * context.scale;
    let period_y = if context.y_up { period } else { -period };
    let grain = crate::ink::host_rgba8::sample_unorm8_half_rtz(sample(
        GRAIN,
        [150, 150],
        [
            (pixel[0] - context.grain_offset[0]) / period,
            (pixel[1] - context.grain_offset[1]) / period_y,
        ],
        false,
    ));
    let dx = fx - 0.5;
    let dy = 0.5 + negative_w;
    let dot = dx * r[7] + dy * r[8];
    let p = (dx * dx + dy * dy - dot * dot).sqrt();
    let factor = 1. / (9. * r[5] + 1.);
    if p > factor {
        return [0; 4];
    }
    // Mesa lowers the shader clamp to fmin then fmax, which select the numeric
    // operand when the other operand is NaN.
    #[allow(clippy::manual_clamp)]
    let mut alpha = (grain * r[4] * ((0.5 - dot) * ((factor - p) / factor)))
        .min(1.)
        .max(0.);
    let distance = (dx * dx + dy * dy).sqrt();
    let mut smooth = r[10];
    if distance > r[9] {
        smooth *= 1. - (distance - r[9]) / (0.5 - r[9]);
    }
    alpha *= smooth;
    if !alpha.is_finite() {
        return [0; 4];
    }
    crate::ink::host_rgba8::premultiply(
        [r[11], r[12], r[13], alpha].map(crate::ink::host_rgba8::quantize),
    )
}

pub(crate) struct RasterPlan<'a> {
    records: &'a [[f32; 14]],
    context: Context,
    stamps: Vec<ProjectedStamp>,
    pixels: usize,
    work: usize,
}

pub(crate) enum RasterError {
    Context,
    Record,
    WorkLimit,
    Allocation,
}

struct ProjectedStamp {
    rect: crate::ink::host_raster::AxisRect<3>,
}

impl ProjectedStamp {
    fn new(r: &[f32; 14], context: Context) -> Result<Self, RasterError> {
        let [width, height] = context.frame.map(|v| v as f32);
        let project = |value: f32, extent: f32, translation: f32, sign: f32| {
            let clip =
                value * (2. * context.scale / extent * sign) + (2. * translation / extent - sign);
            clip.mul_add(extent / 2., extent / 2.)
        };
        let x = [r[0] - r[2], r[0] + r[2]]
            .map(|value| project(value, width, context.translation[0], 1.));
        let y = [r[1] - r[3], r[1] + r[3]].map(|value| {
            project(
                value,
                height,
                context.translation[1],
                if context.y_up { 1. } else { -1. },
            )
        });
        // The captured VS exports U, -W and the already-divided atlas V.
        // Interpolating W and dividing V in the fragment changes texel selection.
        let values = [
            [0., 0., 1., 1.],
            [-1., 0., -1., 0.],
            [r[6] / 3., (1. + r[6]) / 3., r[6] / 3., (1. + r[6]) / 3.],
        ];
        let rect =
            crate::ink::host_raster::AxisRect::new(x, y, context.frame, values, context.y_up)
                .ok_or(RasterError::Record)?;
        Ok(Self { rect })
    }
}

impl RasterPlan<'_> {
    pub(crate) fn work(&self) -> usize {
        self.work
    }

    pub(crate) fn render(self, backdrop: Option<&[[u8; 4]]>) -> Option<Vec<[u8; 4]>> {
        if backdrop.is_some_and(|image| image.len() != self.pixels) {
            return None;
        }
        let mut image = Vec::new();
        image.try_reserve_exact(self.pixels).ok()?;
        if let Some(backdrop) = backdrop {
            image.extend_from_slice(backdrop);
        } else {
            image.resize(self.pixels, [0; 4]);
        }
        let [width, height] = self.context.frame.map(|v| v as usize);
        let context = self.context;
        for (r, stamp) in self.records.iter().zip(self.stamps) {
            let [xa, ya, xb, yb] = stamp.rect.bounds();
            for y in ya..yb {
                for x in xa..xb {
                    let Some(uv) = stamp.rect.sample(x, y) else {
                        continue;
                    };
                    over(
                        &mut image[y * width + x],
                        fragment(
                            r,
                            uv,
                            [x as f32 + 0.5, height as f32 - 0.5 - y as f32],
                            context,
                        ),
                    );
                }
            }
        }
        Some(image)
    }
}

/// Validate the whole batch and bound work before allocating pixel storage.
pub(crate) fn prepare(
    records: &[[f32; 14]],
    context: Context,
) -> Result<RasterPlan<'_>, RasterError> {
    let [width, height] = context.frame.map(|v| v as usize);
    let pixels = width.checked_mul(height).ok_or(RasterError::Context)?;
    if width == 0
        || height == 0
        || pixels > MAX_PIXELS
        || width > 8192
        || height > 8192
        || !context.scale.is_finite()
        || context.scale <= 0.
        || context
            .translation
            .iter()
            .chain(context.grain_offset.iter())
            .any(|v| !v.is_finite())
    {
        return Err(RasterError::Context);
    }
    if records.len() > MAX_RECORDS {
        return Err(RasterError::WorkLimit);
    }
    let mut stamps = Vec::new();
    stamps
        .try_reserve_exact(records.len())
        .map_err(|_| RasterError::Allocation)?;
    let mut samples = 0usize;
    for r in records {
        if r.iter().any(|v| !v.is_finite())
            || r[2] <= 0.
            || r[3] <= 0.
            || r[4] < 0.
            || r[5] < 0.
            || ![0., 1., 2.].contains(&r[6])
            || r[9] >= 0.5
            || !(0. ..=1.).contains(&r[10])
            || r[11..].iter().any(|v| !(0. ..=1.).contains(v))
        {
            return Err(RasterError::Record);
        }
        let stamp = ProjectedStamp::new(r, context)?;
        let rect = stamp.rect.bounds();
        samples = samples
            .checked_add(
                (rect[2] - rect[0])
                    .checked_mul(rect[3] - rect[1])
                    .ok_or(RasterError::WorkLimit)?,
            )
            .ok_or(RasterError::WorkLimit)?;
        if samples > MAX_SAMPLES {
            return Err(RasterError::WorkLimit);
        }
        stamps.push(stamp);
    }
    let work = samples
        .checked_add(pixels)
        .and_then(|n| n.checked_add(records.len()))
        .ok_or(RasterError::WorkLimit)?;
    Ok(RasterPlan {
        records,
        context,
        stamps,
        pixels,
        work,
    })
}

#[cfg(test)]
pub(crate) fn render(records: &[[f32; 14]], context: Context) -> Option<Vec<[u8; 4]>> {
    render_with_backdrop(records, context, None)
}

#[cfg(test)]
pub(crate) fn render_with_backdrop(
    records: &[[f32; 14]],
    context: Context,
    backdrop: Option<&[[u8; 4]]>,
) -> Option<Vec<[u8; 4]>> {
    prepare(records, context).ok()?.render(backdrop)
}

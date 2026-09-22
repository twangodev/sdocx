//! Verified built-in template geometry, shared by diagnostics and SVG output.
use crate::{DocumentMetadata, Page, PageTemplateSource};

#[derive(Debug, Clone, Copy)]
#[cfg_attr(not(feature = "render"), allow(dead_code))]
pub(crate) struct DotPattern {
    pub(crate) pitch_x: f64,
    pub(crate) pitch_y: f64,
    pub(crate) top: f64,
    pub(crate) radius_x: f64,
    pub(crate) radius_y: f64,
    pub(crate) rows: u32,
}

#[derive(Debug, Clone, Copy)]
#[cfg_attr(not(feature = "render"), allow(dead_code))]
pub(crate) struct LinePattern {
    pub(crate) pitch_y: f64,
    pub(crate) first_y: f64,
    pub(crate) width: f64,
    pub(crate) rows: u32,
}

#[derive(Debug, Clone, Copy)]
#[cfg_attr(not(feature = "render"), allow(dead_code))]
pub(crate) enum TemplatePattern {
    Dots(DotPattern),
    Lines(LinePattern),
}

pub(crate) fn template_pattern(
    page: &Page,
    metadata: &DocumentMetadata,
) -> Result<Option<TemplatePattern>, &'static str> {
    let background = &page.background;
    if background
        .template_uri
        .as_ref()
        .is_some_and(|uri| !uri.is_empty())
        || background.image_id.is_some_and(|id| id != u32::MAX)
    {
        return Err("image/URI-backed page backgrounds are not supported");
    }
    let Some(template) = page.template else {
        return Ok(None);
    };
    if template.source != PageTemplateSource::BuiltIn || !matches!(template.id, 1..=3 | 7..=9) {
        return Err("page template is retained but not supported");
    }
    if background.rotation.is_some_and(|rotation| rotation != 0) {
        return Err("rotated built-in backgrounds are not supported");
    }
    let Some((width, height)) = metadata.default_page_dimensions else {
        return Err("page template requires native default page dimensions");
    };
    let axis = match metadata.orientation {
        Some(0) => width,
        Some(1) => height,
        _ => return Err("page template requires a known document orientation"),
    };
    if axis == 0 || page.width == 0 || page.height == 0 {
        return Err("page template has zero dimensions");
    }
    // WNote::GetDocumentDensity; TemplateDrawingBase::GetLineHeight;
    // DotTemplateDrawing::Draw; TemplateDrawing::onDraw (APK 4.4.45.37).
    // Use native f32 arithmetic before expanding to SVG coordinates.
    let density = axis as f32 / 360.0;
    let ruled = matches!(template.id, 1..=3);
    let variant = template.id - if ruled { 1 } else { 7 };
    let size = [12.0_f32, 17.0, 28.0][variant as usize];
    let pitch_y = size * 1.35 * density;
    let tile_height = pitch_y.ceil();
    let scale_y = pitch_y / tile_height;
    let radius = (2.0 * density).max(1.0) / 2.0;
    // Bound SVG work for malformed/extreme page geometry. Native templates
    // normally have tens of rows, not thousands.
    let top = 10.0 * density * scale_y;
    let rows = ((page.height as f64 - f64::from(top)).max(0.0) / f64::from(pitch_y)).ceil();
    if rows > 10_000.0 {
        return Err("page template exceeds the supported row count");
    }
    if ruled {
        // LineTemplateDrawing::Draw uses half a density unit, at least one
        // native pixel at unit zoom, then the same tile-height correction.
        let width = (0.5 * density).max(1.0) * scale_y;
        return Ok(Some(TemplatePattern::Lines(LinePattern {
            pitch_y: f64::from(pitch_y),
            first_y: f64::from(top) + f64::from(width) / 2.,
            width: f64::from(width),
            rows: rows as u32,
        })));
    }
    Ok(Some(TemplatePattern::Dots(DotPattern {
        pitch_x: f64::from(tile_height) + 1.5 * f64::from(density),
        pitch_y: f64::from(pitch_y),
        top: f64::from(top),
        radius_x: f64::from(radius),
        radius_y: f64::from(radius * scale_y),
        rows: rows as u32,
    })))
}

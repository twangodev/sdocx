//! Compare rendered ink inside selected prepared-stroke bounds.
//!
//! Images must use the same dimensions and one pixel per geometry unit. Three
//! pixels of padding retain antialias coverage. Profile/settings filters select
//! regions, not individual overlapping objects; a region mask cannot undo
//! compositing where strokes overlap.

use std::io::{Error, ErrorKind};

use serde_json::{Value, json};

use crate::Result;
use crate::image::{MetricsOptions, RgbImage, measure};

const PADDING: i64 = 3;
const INK_THRESHOLD: u8 = 32;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct InkOptions {
    pub profile: Option<String>,
    pub settings: Option<String>,
    pub region: Option<[i64; 4]>,
    pub chroma_threshold: Option<u8>,
}

/// Build the union mask for selected geometry records and return their count.
pub fn stroke_regions(
    rows: &[Value],
    size: (u32, u32),
    options: &InkOptions,
) -> Result<(Vec<bool>, usize)> {
    let (width, height) = (i64::from(size.0), i64::from(size.1));
    if let Some([x0, y0, x1, y1]) = options.region
        && (x0 < 0 || y0 < 0 || x0 >= x1 || y0 >= y1 || x1 > width || y1 > height)
    {
        return Err(Error::new(
            ErrorKind::InvalidInput,
            "region must be a nonempty rectangle inside the image",
        )
        .into());
    }

    let mut mask = vec![false; size.0 as usize * size.1 as usize];
    let mut count = 0;
    for item in rows {
        let geometry = item.get("geometry").ok_or_else(|| {
            Error::new(ErrorKind::InvalidData, "geometry row has no geometry field")
        })?;
        if options.profile.as_ref().is_some_and(|profile| {
            geometry.get("profile").and_then(Value::as_str) != Some(profile.as_str())
        }) {
            continue;
        }
        if let Some(settings) = options.settings.as_ref() {
            let advanced = item
                .get("stroke")
                .and_then(|stroke| stroke.get("rendering"))
                .and_then(|rendering| rendering.get("advanced_settings"))
                .and_then(Value::as_str);
            if advanced != Some(settings.as_str()) {
                continue;
            }
        }

        let bounds = geometry.get("bounds").ok_or_else(|| {
            Error::new(ErrorKind::InvalidData, "geometry row has no bounds field")
        })?;
        if bounds.is_null() {
            continue;
        }
        let coordinate = |name: &str| -> Result<f64> {
            bounds.get(name).and_then(Value::as_f64).ok_or_else(|| {
                Error::new(
                    ErrorKind::InvalidData,
                    format!("stroke bounds have no numeric {name}"),
                )
                .into()
            })
        };
        let (x_min, y_min, x_max, y_max) = (
            coordinate("x_min")?,
            coordinate("y_min")?,
            coordinate("x_max")?,
            coordinate("y_max")?,
        );
        if ![x_min, y_min, x_max, y_max].into_iter().all(f64::is_finite) {
            return Err(Error::new(ErrorKind::InvalidData, "non-finite stroke bounds").into());
        }
        if x_min > x_max || y_min > y_max {
            return Err(Error::new(ErrorKind::InvalidData, "inverted stroke bounds").into());
        }
        let mut x0 = (x_min.floor() as i64).saturating_sub(PADDING);
        let mut y0 = (y_min.floor() as i64).saturating_sub(PADDING);
        let mut x1 = (x_max.ceil() as i64).saturating_add(PADDING);
        let mut y1 = (y_max.ceil() as i64).saturating_add(PADDING);

        if let Some([region_x0, region_y0, region_x1, region_y1]) = options.region {
            x0 = x0.max(region_x0);
            y0 = y0.max(region_y0);
            x1 = x1.min(region_x1);
            y1 = y1.min(region_y1);
            if x0 >= x1 || y0 >= y1 {
                continue;
            }
        }
        if x1 < 0 || y1 < 0 || x0 >= width || y0 >= height {
            return Err(Error::new(
                ErrorKind::InvalidInput,
                "stroke bounds outside image; check page and coordinate scale",
            )
            .into());
        }

        for y in y0.max(0)..y1.min(height) {
            let row_start = y as usize * size.0 as usize;
            for x in x0.max(0)..x1.min(width) {
                mask[row_start + x as usize] = true;
            }
        }
        count += 1;
    }

    if count == 0 {
        return Err(Error::new(ErrorKind::InvalidInput, "no selected stroke regions").into());
    }
    Ok((mask, count))
}

/// Apply the stroke mask and optional chroma filter, replacing excluded pixels
/// with white so the shared comparison metrics only see selected ink.
pub fn masked_ink(
    image: &RgbImage,
    mask: &[bool],
    chroma_threshold: Option<u8>,
) -> Result<RgbImage> {
    if mask.len() != image.pixels.len() {
        return Err(
            Error::new(ErrorKind::InvalidInput, "mask dimensions differ from image").into(),
        );
    }
    if chroma_threshold.is_some_and(|threshold| threshold == 255) {
        return Err(Error::new(
            ErrorKind::InvalidInput,
            "chroma threshold must be in 0..254",
        )
        .into());
    }
    let mut selected = image.clone();
    for (index, pixel) in selected.pixels.iter_mut().enumerate() {
        let chroma = *pixel.iter().max().unwrap() - *pixel.iter().min().unwrap();
        if !mask[index] || chroma_threshold.is_some_and(|threshold| chroma <= threshold) {
            *pixel = [255; 3];
        }
    }
    Ok(selected)
}

/// Measure masked rendered ink and return the Python tool's JSON fields.
pub fn compare(
    rows: &[Value],
    reference: &RgbImage,
    candidate: &RgbImage,
    options: &InkOptions,
) -> Result<Value> {
    if (reference.width, reference.height) != (candidate.width, candidate.height) {
        return Err(Error::new(
            ErrorKind::InvalidInput,
            "images must have matching dimensions",
        )
        .into());
    }
    if options
        .chroma_threshold
        .is_some_and(|threshold| threshold == 255)
    {
        return Err(Error::new(
            ErrorKind::InvalidInput,
            "chroma threshold must be in 0..254",
        )
        .into());
    }
    let (mask, stroke_count) = stroke_regions(rows, (reference.width, reference.height), options)?;
    let reference_ink = masked_ink(reference, &mask, options.chroma_threshold)?;
    let candidate_ink = masked_ink(candidate, &mask, options.chroma_threshold)?;
    let (metrics, _) = measure(&reference_ink, &candidate_ink, MetricsOptions::default())?;

    let mut common = 0usize;
    let mut union = 0usize;
    let mut common_channel_error = 0u64;
    for (reference_pixel, candidate_pixel) in reference_ink.pixels.iter().zip(&candidate_ink.pixels)
    {
        let reference_is_ink = reference_pixel.iter().min().unwrap() < &(255 - INK_THRESHOLD);
        let candidate_is_ink = candidate_pixel.iter().min().unwrap() < &(255 - INK_THRESHOLD);
        common += usize::from(reference_is_ink && candidate_is_ink);
        union += usize::from(reference_is_ink || candidate_is_ink);
        if reference_is_ink && candidate_is_ink {
            common_channel_error += reference_pixel
                .iter()
                .zip(candidate_pixel)
                .map(|(left, right)| u64::from(left.abs_diff(*right)))
                .sum::<u64>();
        }
    }
    let ink_metrics = serde_json::to_value(metrics)?;
    let ink_metrics = ink_metrics
        .as_object()
        .expect("PixelMetrics serializes as an object")
        .iter()
        .filter(|(name, _)| name.contains("ink"))
        .map(|(name, value)| (name.clone(), value.clone()))
        .collect::<serde_json::Map<_, _>>();

    let mut output = json!({
        "stroke_regions": stroke_count,
        "padding": PADDING,
        "tolerance": 1,
        "ink_threshold": INK_THRESHOLD,
        "profile": options.profile.as_deref(),
        "settings": options.settings.as_deref(),
        "region": options.region,
        "chroma_threshold": options.chroma_threshold,
        "ink_intersection_over_union": if union == 0 { None } else { Some(common as f64 / union as f64) },
        "shared_ink_mean_rgb_error": if common == 0 { None } else { Some(common_channel_error as f64 / (3.0 * common as f64)) },
    });
    let output = output
        .as_object_mut()
        .expect("ink output serializes as an object");
    output.extend(ink_metrics);
    Ok(Value::Object(output.clone()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stroke(profile: &str, settings: &str, x: f64, y: f64) -> Value {
        json!({
            "stroke": {"rendering": {"advanced_settings": settings}},
            "geometry": {"profile": profile, "bounds": {
                "x_min": x, "y_min": y, "x_max": x + 2.0, "y_max": y + 2.0
            }}
        })
    }

    #[test]
    fn profile_settings_and_region_select_only_requested_strokes() {
        let rows = [
            stroke("Marker4", "7;", 10.0, 10.0),
            stroke("Marker4", "8;", 30.0, 10.0),
            stroke("FountainPen", "14;", 10.0, 30.0),
            stroke("Marker4", "7;", 10.0, 100.0),
        ];
        let options = InkOptions {
            profile: Some("Marker4".into()),
            settings: Some("7;".into()),
            region: Some([0, 0, 50, 20]),
            chroma_threshold: None,
        };
        let (mask, count) = stroke_regions(&rows, (50, 50), &options).unwrap();
        assert_eq!(count, 1);
        assert!(mask[10 * 50 + 10]);
        assert!(!mask[10 * 50 + 30]);
        assert!(!mask[30 * 50 + 10]);
        assert!(
            stroke_regions(&rows, (50, 50), &InkOptions::default())
                .unwrap_err()
                .to_string()
                .contains("outside image")
        );
        let missing = InkOptions {
            profile: Some("InkPen2".into()),
            ..InkOptions::default()
        };
        assert!(
            stroke_regions(&rows, (50, 50), &missing)
                .unwrap_err()
                .to_string()
                .contains("no selected")
        );
    }

    #[test]
    fn invalid_regions_are_rejected_before_selecting_rows() {
        for region in [[0, 0, 0, 10], [-1, 0, 20, 20], [0, 0, 51, 50]] {
            let options = InkOptions {
                region: Some(region),
                ..InkOptions::default()
            };
            assert!(
                stroke_regions(&[], (50, 50), &options)
                    .unwrap_err()
                    .to_string()
                    .contains("region")
            );
        }
    }

    #[test]
    fn inverted_bounds_are_rejected_before_padding_but_zero_extent_is_allowed() {
        let inverted = [
            (12.0, 10.0, 11.0, 12.0),
            (10.0, 12.0, 12.0, 11.0),
            (100.0, 10.0, -100.0, 12.0),
            (10.0, 100.0, 12.0, -100.0),
        ];
        for (x_min, y_min, x_max, y_max) in inverted {
            let row = json!({
                "geometry": {"profile": "Marker4", "bounds": {
                    "x_min": x_min, "y_min": y_min,
                    "x_max": x_max, "y_max": y_max
                }}
            });
            assert!(
                stroke_regions(&[row], (50, 50), &InkOptions::default())
                    .unwrap_err()
                    .to_string()
                    .contains("inverted")
            );
        }

        let zero_width = json!({
            "geometry": {"profile": "Marker4", "bounds": {
                "x_min": 12.0, "y_min": 10.0,
                "x_max": 12.0, "y_max": 12.0
            }}
        });
        let (mask, count) =
            stroke_regions(&[zero_width], (50, 50), &InkOptions::default()).unwrap();
        assert_eq!(count, 1);
        assert!(mask[10 * 50 + 12]);
    }

    #[test]
    fn chroma_filter_removes_monochrome_and_alpha_is_composited_before_filtering() {
        let image = RgbImage {
            width: 3,
            height: 1,
            pixels: [
                [0, 0, 0],
                crate::image::composite_white([50, 150, 255], 255),
                crate::image::composite_white([50, 150, 255], 0),
            ]
            .into(),
        };
        let mask = [true, true, true];
        let filtered = masked_ink(&image, &mask, Some(35)).unwrap();
        assert_eq!(filtered.pixels, [[255; 3], [50, 150, 255], [255; 3]]);
        let mut excluded = mask;
        excluded[1] = false;
        assert_eq!(
            masked_ink(&image, &excluded, Some(35)).unwrap().pixels,
            [[255; 3]; 3]
        );
        assert!(
            masked_ink(&image, &mask, Some(255))
                .unwrap_err()
                .to_string()
                .contains("chroma")
        );
        assert_eq!(crate::image::composite_white([0, 0, 0], 0), [255; 3]);
    }
}

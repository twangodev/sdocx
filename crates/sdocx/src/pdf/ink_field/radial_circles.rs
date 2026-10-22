//! Bounded vector circles for the full-coverage Fountain V17 rainbow branch.
//!
//! A native full-coverage stamp is represented by one ordered ellipse fill.
//! This preserves fractional RGB and saved stroke opacity in an isolated
//! transparency Form; smaller stamps stay on the original SVG radial field.

use std::fmt::Write as _;

pub(crate) type PdfRadialCircleRecord = [f32; 9];

pub(crate) struct PdfRadialCircleProgram {
    pub content: Vec<u8>,
    pub alpha: f32,
}

const MAX_RECORDS: usize = 4096;
const MAX_PATHS: usize = 4096;
const MAX_STREAM_BYTES: usize = 1_048_576;
const KAPPA: f64 = 0.552_284_749_830_793_6;

pub(crate) fn compile(
    records: &[PdfRadialCircleRecord],
    bounds: [f64; 4],
    saved_alpha: f32,
) -> Result<PdfRadialCircleProgram, String> {
    if records.is_empty() || records.len() > MAX_RECORDS || records.len() > MAX_PATHS {
        return Err("radial circle record or path count is outside the supported range".into());
    }
    let [x_min, y_min, x_max, y_max] = bounds;
    if !bounds.into_iter().all(f64::is_finite) || x_max <= x_min || y_max <= y_min {
        return Err("radial circle bounds are invalid".into());
    }
    if !saved_alpha.is_finite() || !(0.0..=1.0).contains(&saved_alpha) {
        return Err("radial circle saved alpha is outside [0,1]".into());
    }
    let width = x_max - x_min;
    let height = y_max - y_min;
    if !width.is_finite() || !height.is_finite() {
        return Err("radial circle bounds have an unrepresentable extent".into());
    }

    // Validate every complete circle before serializing any output. Registry
    // insertion follows this compiler, so failure cannot publish a marker.
    for record in records {
        if !record.iter().all(|value| value.is_finite()) {
            return Err("radial circle record contains a non-finite value".into());
        }
        let [cx, cy, rx, ry, edge, coverage, red, green, blue] = *record;
        if rx <= 0.0 || ry <= 0.0 || rx.to_bits() != ry.to_bits() {
            return Err("radial circles require positive equal radii".into());
        }
        if edge != 0.0 || coverage != 1.0 {
            return Err("radial circles require the full-coverage source branch".into());
        }
        if [red, green, blue]
            .into_iter()
            .any(|channel| !(0.0..=1.0).contains(&channel))
        {
            return Err("radial circle RGB is outside [0,1]".into());
        }
        if f64::from(cx) - f64::from(rx) < x_min
            || f64::from(cx) + f64::from(rx) > x_max
            || f64::from(cy) - f64::from(ry) < y_min
            || f64::from(cy) + f64::from(ry) > y_max
        {
            return Err("radial circle bounds do not contain every circle".into());
        }
    }

    let mut content = String::new();
    if saved_alpha > 0.0 {
        for record in records {
            let [cx, cy, rx, ry, _, _, red, green, blue] = *record;
            let cx = (f64::from(cx) - x_min) / width;
            let cy = 1.0 - (f64::from(cy) - y_min) / height;
            let rx = f64::from(rx) / width;
            let ry = f64::from(ry) / height;
            writeln!(
                content,
                "q /GS0 gs {} {} {} rg",
                pdf_number(f64::from(red)),
                pdf_number(f64::from(green)),
                pdf_number(f64::from(blue)),
            )
            .expect("writing to String cannot fail");
            write_ellipse(&mut content, cx, cy, rx, ry);
            content.push_str("f Q\n");
            if content.len() > MAX_STREAM_BYTES {
                return Err("radial circle content exceeds the 1 MiB budget".into());
            }
        }
    }
    if content.len() > MAX_STREAM_BYTES {
        return Err("radial circle content exceeds the 1 MiB budget".into());
    }
    Ok(PdfRadialCircleProgram {
        content: content.into_bytes(),
        alpha: saved_alpha,
    })
}

fn write_ellipse(content: &mut String, cx: f64, cy: f64, rx: f64, ry: f64) {
    let kx = rx * KAPPA;
    let ky = ry * KAPPA;
    write!(content, "{} {} m ", pdf_number(cx + rx), pdf_number(cy))
        .expect("writing to String cannot fail");
    write!(
        content,
        "{} {} {} {} {} {} c ",
        pdf_number(cx + rx),
        pdf_number(cy + ky),
        pdf_number(cx + kx),
        pdf_number(cy + ry),
        pdf_number(cx),
        pdf_number(cy + ry),
    )
    .expect("writing to String cannot fail");
    write!(
        content,
        "{} {} {} {} {} {} c ",
        pdf_number(cx - kx),
        pdf_number(cy + ry),
        pdf_number(cx - rx),
        pdf_number(cy + ky),
        pdf_number(cx - rx),
        pdf_number(cy),
    )
    .expect("writing to String cannot fail");
    write!(
        content,
        "{} {} {} {} {} {} c ",
        pdf_number(cx - rx),
        pdf_number(cy - ky),
        pdf_number(cx - kx),
        pdf_number(cy - ry),
        pdf_number(cx),
        pdf_number(cy - ry),
    )
    .expect("writing to String cannot fail");
    write!(
        content,
        "{} {} {} {} {} {} c ",
        pdf_number(cx + kx),
        pdf_number(cy - ry),
        pdf_number(cx + rx),
        pdf_number(cy - ky),
        pdf_number(cx + rx),
        pdf_number(cy),
    )
    .expect("writing to String cannot fail");
    content.push_str("h\n");
}

fn pdf_number(value: f64) -> String {
    if value == 0.0 {
        return "0".into();
    }
    let scientific = format!("{value:.17e}");
    let (mantissa, exponent) = scientific
        .split_once('e')
        .expect("scientific formatter emits an exponent");
    let exponent = exponent.parse::<i32>().expect("integer exponent");
    let negative = mantissa.starts_with('-');
    let digits = mantissa
        .trim_start_matches('-')
        .chars()
        .filter(|character| *character != '.')
        .collect::<String>();
    let point = 1 + exponent;
    let fixed = if point <= 0 {
        format!("0.{}{}", "0".repeat((-point) as usize), digits)
    } else if point as usize >= digits.len() {
        format!("{}{}", digits, "0".repeat(point as usize - digits.len()))
    } else {
        let point = point as usize;
        format!("{}.{}", &digits[..point], &digits[point..])
    };
    let fixed = if fixed.contains('.') {
        fixed.trim_end_matches('0').trim_end_matches('.').to_owned()
    } else {
        fixed
    };
    if negative { format!("-{fixed}") } else { fixed }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record(x: f32, color: [f32; 3]) -> PdfRadialCircleRecord {
        [x, 5.0, 1.5, 1.5, 0.0, 1.0, color[0], color[1], color[2]]
    }

    fn bounds() -> [f64; 4] {
        [2.0, 2.0, 8.0, 8.0]
    }

    #[test]
    fn emits_one_ordered_fractional_rgb_circle_per_record_with_saved_alpha_once() {
        let records = [record(4.0, [0.25, 0.5, 0.75]), record(6.0, [0.8, 0.1, 0.2])];
        let program = compile(&records, bounds(), 0.5).unwrap();
        let content = String::from_utf8(program.content).unwrap();
        let colors: Vec<Vec<f32>> = lopdf::content::Content::decode(content.as_bytes())
            .unwrap()
            .operations
            .iter()
            .filter(|operation| operation.operator == "rg")
            .map(|operation| {
                operation
                    .operands
                    .iter()
                    .map(|value| value.as_float().unwrap())
                    .collect()
            })
            .collect();
        assert_eq!(colors, vec![vec![0.25, 0.5, 0.75], vec![0.8, 0.1, 0.2]]);
        assert_eq!(content.matches(" /GS0 gs ").count(), 2);
        assert_eq!(content.matches(" c ").count(), 8);
        assert_eq!(program.alpha, 0.5);
    }

    #[test]
    fn zero_opacity_produces_an_empty_transparent_program() {
        let program = compile(&[record(5.0, [0.2, 0.4, 0.6])], bounds(), 0.0).unwrap();
        assert!(program.content.is_empty());
        assert_eq!(program.alpha, 0.0);
    }

    #[test]
    fn rejects_invalid_records_bounds_and_budgets_before_producing_content() {
        let mut invalid = record(5.0, [0.2, 0.4, 0.6]);
        invalid[2] = 1.0;
        assert!(compile(&[invalid], bounds(), 1.0).is_err());
        invalid = record(5.0, [0.2, 0.4, 0.6]);
        invalid[4] = 0.1;
        assert!(compile(&[invalid], bounds(), 1.0).is_err());
        invalid = record(5.0, [0.2, 0.4, 0.6]);
        invalid[5] = 0.5;
        assert!(compile(&[invalid], bounds(), 1.0).is_err());
        invalid = record(5.0, [0.2, 0.4, 0.6]);
        invalid[6] = f32::NAN;
        assert!(compile(&[invalid], bounds(), 1.0).is_err());
        assert!(compile(&[record(5.0, [0.2, 0.4, 0.6])], [4.0, 4.0, 6.0, 6.0], 1.0).is_err());
        assert!(
            compile(
                &vec![record(5.0, [0.2, 0.4, 0.6]); MAX_RECORDS + 1],
                bounds(),
                1.0
            )
            .is_err()
        );
        assert!(compile(&[record(5.0, [0.2, 0.4, 0.6])], bounds(), 1.1).is_err());

        let many = vec![record(5.0, [0.2, 0.4, 0.6]); MAX_RECORDS];
        let serialized_bounds = [2.0, 2.0, 8.0, 8.0];
        assert!(
            compile(&many, serialized_bounds, 1.0).is_err(),
            "1 MiB content cap is enforced"
        );
    }
}

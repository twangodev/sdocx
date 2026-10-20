//! PDF Type 4 programs for per-record radial ink fields.
//!
//! Records retain the native nine-f32 layout:
//! `[cx, cy, rx, ry, edge, coverage, r, g, b]`.

pub(crate) type PdfRadialRecord = [f32; 9];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PdfRadialCoverage {
    /// Native direct route: ordered per-stamp source-over coverage `q`.
    DirectSourceOver,
    /// Native enhanced route: maximum stamp coverage `M`.
    EnhancedMax,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct PdfRadialProgram {
    pub rgb: Vec<u8>,
    pub mask: Vec<u8>,
}

const MAX_RECORDS: usize = 512;
const MAX_CALCULATOR_BYTES: usize = 1_048_576;

/// Compile the two bounded Type 4 programs in unit form coordinates.
///
/// Bounds are the existing marker's page-space bounds. The caller maps its
/// `[0,1]²` marker Form to those bounds, so centers and radii are normalized
/// independently on each axis.
pub(crate) fn compile(
    records: &[PdfRadialRecord],
    bounds: [f64; 4],
    saved_alpha: f32,
    coverage: PdfRadialCoverage,
) -> Result<PdfRadialProgram, String> {
    if records.is_empty() || records.len() > MAX_RECORDS {
        return Err("radial ink record count is outside the supported range".into());
    }
    if !bounds.into_iter().all(f64::is_finite) || bounds[2] <= bounds[0] || bounds[3] <= bounds[1] {
        return Err("radial ink bounds are invalid".into());
    }
    if !saved_alpha.is_finite() || !(0.0..=1.0).contains(&saved_alpha) {
        return Err("radial ink alpha is outside [0,1]".into());
    }
    let width = bounds[2] - bounds[0];
    let height = bounds[3] - bounds[1];
    if !width.is_finite() || !height.is_finite() {
        return Err("radial ink bounds have an unrepresentable extent".into());
    }

    let mut normalized = Vec::with_capacity(records.len());
    for record in records {
        if !record.iter().all(|value| value.is_finite()) {
            return Err("radial ink record contains a non-finite value".into());
        }
        let [cx, cy, rx, ry, edge, cov, red, green, blue] = *record;
        if rx <= 0.0 || ry <= 0.0 {
            return Err("radial ink radii must be positive".into());
        }
        if !(0.0..=1.0).contains(&cov) || !(0.0..0.5).contains(&edge) {
            return Err("radial ink edge must be in [0,0.5) and coverage in [0,1]".into());
        }
        // DeviceRGB and the native RGBA8 target both clamp at the final color
        // boundary. A normalized straight-RGB PDF function cannot preserve
        // out-of-range native source colors before alpha is applied, so fail
        // closed for those profiles.
        if [red, green, blue]
            .into_iter()
            .any(|channel| !(0.0..=1.0).contains(&channel))
        {
            return Err("radial ink RGB is outside the PDF DeviceRGB range".into());
        }
        let normalized_record = [
            (f64::from(cx) - bounds[0]) / width,
            1.0 - (f64::from(cy) - bounds[1]) / height,
            f64::from(rx) / width,
            f64::from(ry) / height,
            f64::from(edge),
            f64::from(cov),
            f64::from(red),
            f64::from(green),
            f64::from(blue),
        ];
        if !normalized_record.iter().all(|value| value.is_finite())
            || normalized_record[2] <= 0.0
            || normalized_record[3] <= 0.0
        {
            return Err("radial ink record cannot be normalized to its marker bounds".into());
        }
        if f64::from(cx) - f64::from(rx) < bounds[0]
            || f64::from(cx) + f64::from(rx) > bounds[2]
            || f64::from(cy) - f64::from(ry) < bounds[1]
            || f64::from(cy) + f64::from(ry) > bounds[3]
        {
            return Err("radial ink bounds do not contain every ellipse".into());
        }
        normalized.push(normalized_record);
    }

    let alpha = f64::from(saved_alpha);
    let rgb_code = compile_rgb(&normalized, alpha);
    let mask_code = match coverage {
        PdfRadialCoverage::DirectSourceOver => compile_q(&normalized, alpha),
        PdfRadialCoverage::EnhancedMax => compile_max(&normalized),
    };
    if rgb_code.len().saturating_add(mask_code.len()) > MAX_CALCULATOR_BYTES {
        return Err("radial ink Type 4 programs exceed the byte budget".into());
    }
    Ok(PdfRadialProgram {
        rgb: rgb_code,
        mask: mask_code,
    })
}

fn pdf_number(value: f64) -> String {
    // Use 18 significant decimal digits, then expand any exponent to PDF's
    // fixed-decimal number grammar. Exponent notation is not valid in Type 4
    // calculator code. This preserves f32 inputs and normalized f64 ratios.
    let scientific = format!("{value:.17e}");
    let (mantissa, exponent) = scientific
        .split_once('e')
        .expect("Rust scientific formatting always contains an exponent");
    let exponent = exponent
        .parse::<i32>()
        .expect("formatted exponent is an integer");
    let negative = mantissa.starts_with('-');
    let digits = mantissa
        .trim_start_matches('-')
        .chars()
        .filter(|character| *character != '.')
        .collect::<String>();
    let point = 1 + exponent;
    let mut fixed = if point <= 0 {
        format!("0.{}{}", "0".repeat((-point) as usize), digits)
    } else if point as usize >= digits.len() {
        format!("{}{}", digits, "0".repeat(point as usize - digits.len()))
    } else {
        let point = point as usize;
        format!("{}.{}", &digits[..point], &digits[point..])
    };
    if fixed.contains('.') {
        while fixed.ends_with('0') {
            fixed.pop();
        }
        if fixed.ends_with('.') {
            fixed.pop();
        }
    }
    if fixed
        .chars()
        .all(|character| character == '0' || character == '.')
    {
        return "0".into();
    }
    if negative { format!("-{fixed}") } else { fixed }
}

fn coverage_expression(record: &[f64; 9], xy_depth: usize) -> String {
    let [cx, cy, rx, ry, edge, coverage, ..] = *record;
    let x_depth = xy_depth;
    format!(
        "{x_depth} index {} sub {} div dup mul {x_depth} index {} sub {} div dup mul add sqrt dup {} le {{ pop {} }} {{ dup 0.5 ge {{ pop 0 }} {{ 0.5 exch sub {} div {} mul }} ifelse }} ifelse",
        pdf_number(cx),
        pdf_number(2.0 * rx),
        pdf_number(cy),
        pdf_number(2.0 * ry),
        pdf_number(edge),
        pdf_number(coverage),
        pdf_number(0.5 - edge),
        pdf_number(coverage),
    )
}

fn compile_max(records: &[[f64; 9]]) -> Vec<u8> {
    let mut code = String::from("{ 0 ");
    for record in records {
        code.push_str(&coverage_expression(record, 2));
        // x y old_max coverage -> x y max(old_max, coverage)
        code.push_str(" 2 copy gt { pop } { exch pop } ifelse ");
    }
    code.push_str("exch pop exch pop }");
    code.into_bytes()
}

fn compile_q(records: &[[f64; 9]], alpha: f64) -> Vec<u8> {
    let mut code = String::from("{ 0 ");
    for record in records {
        code.push_str(&coverage_expression(record, 2));
        code.push_str(&format!(" {} mul ", pdf_number(alpha)));
        // x y old_q a -> x y old_q + a * (1-old_q), then discard old state.
        code.push_str("1 index 1 exch sub 1 index mul 2 index add 3 1 roll pop pop ");
    }
    code.push_str("exch pop exch pop }");
    code.into_bytes()
}

fn compile_rgb(records: &[[f64; 9]], alpha: f64) -> Vec<u8> {
    // Constant stack independent of stamp count: x y P_R P_G P_B q.
    let mut code = String::from("{ 0 0 0 0 ");
    for record in records {
        code.push_str(&coverage_expression(record, 5));
        code.push_str(&format!(" {} mul ", pdf_number(alpha)));
        for channel in 0..3 {
            let source_channel = pdf_number(record[6 + channel]);
            code.push_str(&format!(
                "4 index {} index 1 exch sub mul {} index {} mul add ",
                channel + 1,
                channel + 1,
                source_channel,
            ));
        }
        code.push_str("4 index 4 index 1 exch sub mul 4 index add 9 4 roll pop pop pop pop pop ");
    }
    code.push_str(
        "dup 0 gt { 3 index 1 index div 3 index 2 index div 3 index 3 index div 9 3 roll pop pop pop pop pop pop } { pop pop pop pop pop pop 0 0 0 } ifelse }",
    );
    code.into_bytes()
}

/// Independent CPU definition for generated-program comparisons.
#[cfg(test)]
fn evaluate(
    records: &[PdfRadialRecord],
    saved_alpha: f32,
    mode: PdfRadialCoverage,
    x: f64,
    y: f64,
) -> [f64; 4] {
    let mut premultiplied = [0.0; 3];
    let mut q = 0.0;
    let mut max_coverage: f64 = 0.0;
    for record in records {
        let [cx, cy, rx, ry, edge, coverage, red, green, blue] = *record;
        let distance = (((x - f64::from(cx)) / (2.0 * f64::from(rx))).powi(2)
            + ((y - f64::from(cy)) / (2.0 * f64::from(ry))).powi(2))
        .sqrt();
        let c = if distance <= f64::from(edge) {
            f64::from(coverage)
        } else if distance >= 0.5 {
            0.0
        } else {
            f64::from(coverage) * (0.5 - distance) / (0.5 - f64::from(edge))
        };
        let a = c * f64::from(saved_alpha);
        max_coverage = max_coverage.max(c);
        for (channel, color) in [red, green, blue].into_iter().enumerate() {
            premultiplied[channel] = f64::from(color) * a + premultiplied[channel] * (1.0 - a);
        }
        q = a + q * (1.0 - a);
    }
    let mut result = [0.0; 4];
    if q > 0.0 {
        for channel in 0..3 {
            result[channel] = premultiplied[channel] / q;
        }
    }
    result[3] = match mode {
        PdfRadialCoverage::DirectSourceOver => q,
        PdfRadialCoverage::EnhancedMax => max_coverage * f64::from(saved_alpha),
    };
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, Clone)]
    enum PsToken {
        Number(f64),
        Word(String),
        Procedure(Vec<PsToken>),
    }

    fn tokenize(code: &str) -> Vec<PsToken> {
        fn group(tokens: &[&str], cursor: &mut usize, nested: bool) -> Vec<PsToken> {
            let mut result = Vec::new();
            while *cursor < tokens.len() {
                let token = tokens[*cursor];
                *cursor += 1;
                match token {
                    "{" => result.push(PsToken::Procedure(group(tokens, cursor, true))),
                    "}" if nested => return result,
                    "}" => panic!("unexpected closing procedure"),
                    _ => {
                        if let Ok(number) = token.parse::<f64>() {
                            result.push(PsToken::Number(number));
                        } else {
                            result.push(PsToken::Word(token.to_owned()));
                        }
                    }
                }
            }
            assert!(!nested, "unterminated calculator procedure");
            result
        }

        let tokens = code.split_whitespace().collect::<Vec<_>>();
        let mut cursor = 0;
        group(&tokens, &mut cursor, false)
    }

    fn execute(tokens: &[PsToken], stack: &mut Vec<PsToken>, maximum: &mut usize) {
        macro_rules! number {
            () => {{
                match stack.pop().expect("calculator stack underflow") {
                    PsToken::Number(value) => value,
                    other => panic!("expected number, got {other:?}"),
                }
            }};
        }
        for token in tokens {
            match token {
                PsToken::Number(value) => stack.push(PsToken::Number(*value)),
                PsToken::Procedure(_) => stack.push(token.clone()),
                PsToken::Word(word) => match word.as_str() {
                    "index" => {
                        let index = number!() as usize;
                        let value = stack[stack.len() - index - 1].clone();
                        stack.push(value);
                    }
                    "pop" => {
                        stack.pop().expect("calculator stack underflow");
                    }
                    "dup" => stack.push(stack.last().expect("calculator stack underflow").clone()),
                    "copy" => {
                        let count = number!() as usize;
                        let start = stack.len() - count;
                        let copies = stack[start..].to_vec();
                        stack.extend(copies);
                    }
                    "exch" => {
                        let len = stack.len();
                        stack.swap(len - 1, len - 2);
                    }
                    "roll" => {
                        let shift = number!() as isize;
                        let count = number!() as usize;
                        let start = stack.len() - count;
                        stack[start..].rotate_right(shift.rem_euclid(count as isize) as usize);
                    }
                    "add" | "sub" | "mul" | "div" => {
                        let right = number!();
                        let left = number!();
                        let value = match word.as_str() {
                            "add" => left + right,
                            "sub" => left - right,
                            "mul" => left * right,
                            "div" => left / right,
                            _ => unreachable!(),
                        };
                        stack.push(PsToken::Number(value));
                    }
                    "sqrt" => {
                        let value = number!().sqrt();
                        stack.push(PsToken::Number(value));
                    }
                    "gt" | "ge" | "le" => {
                        let right = number!();
                        let left = number!();
                        let value = match word.as_str() {
                            "gt" => left > right,
                            "ge" => left >= right,
                            "le" => left <= right,
                            _ => unreachable!(),
                        };
                        stack.push(PsToken::Word(if value { "true" } else { "false" }.into()));
                    }
                    "if" => {
                        let procedure = match stack.pop().unwrap() {
                            PsToken::Procedure(procedure) => procedure,
                            other => panic!("expected procedure, got {other:?}"),
                        };
                        let condition = match stack.pop().unwrap() {
                            PsToken::Word(value) => value == "true",
                            other => panic!("expected boolean, got {other:?}"),
                        };
                        if condition {
                            execute(&procedure, stack, maximum);
                        }
                    }
                    "ifelse" => {
                        let otherwise = match stack.pop().unwrap() {
                            PsToken::Procedure(procedure) => procedure,
                            other => panic!("expected procedure, got {other:?}"),
                        };
                        let then = match stack.pop().unwrap() {
                            PsToken::Procedure(procedure) => procedure,
                            other => panic!("expected procedure, got {other:?}"),
                        };
                        let condition = match stack.pop().unwrap() {
                            PsToken::Word(value) => value == "true",
                            other => panic!("expected boolean, got {other:?}"),
                        };
                        execute(if condition { &then } else { &otherwise }, stack, maximum);
                    }
                    _ => panic!("unsupported calculator token {word}"),
                },
            }
            *maximum = (*maximum).max(stack.len());
        }
    }

    fn run_calculator(code: &[u8], x: f64, y: f64) -> (Vec<f64>, usize) {
        let source = std::str::from_utf8(code).unwrap();
        let tokens = tokenize(source);
        let [PsToken::Procedure(procedure)] = tokens.as_slice() else {
            panic!("Type 4 calculator must be one outer procedure");
        };
        let mut stack = vec![PsToken::Number(x), PsToken::Number(y)];
        let mut maximum = stack.len();
        execute(procedure, &mut stack, &mut maximum);
        let output = stack
            .into_iter()
            .map(|token| match token {
                PsToken::Number(value) => value,
                other => panic!("unexpected final calculator value {other:?}"),
            })
            .collect();
        (output, maximum)
    }

    fn bounds() -> [f64; 4] {
        [0.0, 0.0, 1.0, 1.0]
    }

    fn overlapping() -> [PdfRadialRecord; 2] {
        [
            [0.5, 0.5, 0.25, 0.25, 0.0, 0.2, 1.0, 0.0, 0.0],
            [0.5, 0.5, 0.25, 0.25, 0.0, 0.3, 0.0, 0.0, 1.0],
        ]
    }

    #[test]
    fn direct_and_enhanced_composition_have_distinct_alpha_contracts() {
        let records = overlapping();
        let direct = evaluate(&records, 0.5, PdfRadialCoverage::DirectSourceOver, 0.5, 0.5);
        let enhanced = evaluate(&records, 0.5, PdfRadialCoverage::EnhancedMax, 0.5, 0.5);
        let first_alpha = f64::from(0.2_f32) * 0.5;
        let second_alpha = f64::from(0.3_f32) * 0.5;
        let direct_alpha = second_alpha + first_alpha * (1.0 - second_alpha);
        let enhanced_alpha = f64::from(0.3_f32) * 0.5;
        assert!((direct[3] - direct_alpha).abs() < 1e-12);
        assert!((enhanced[3] - enhanced_alpha).abs() < 1e-12);
        assert!((direct[0] - first_alpha * (1.0 - second_alpha) / direct_alpha).abs() < 1e-12);
        assert!((direct[2] - second_alpha / direct_alpha).abs() < 1e-12);
        assert_eq!(&direct[..3], &enhanced[..3]);
        assert_ne!(
            evaluate(&records, 0.5, PdfRadialCoverage::DirectSourceOver, 0.5, 0.5)[..3],
            evaluate(
                &[records[1], records[0]],
                0.5,
                PdfRadialCoverage::DirectSourceOver,
                0.5,
                0.5
            )[..3]
        );
    }

    #[test]
    fn zero_alpha_is_transparent_and_zero_coverage_has_black_rgb() {
        let records = overlapping();
        for mode in [
            PdfRadialCoverage::DirectSourceOver,
            PdfRadialCoverage::EnhancedMax,
        ] {
            assert_eq!(evaluate(&records, 0.0, mode, 0.5, 0.5), [0.0; 4]);
            assert_eq!(evaluate(&records, 0.5, mode, 0.0, 0.0), [0.0; 4]);
        }
    }

    #[test]
    fn twenty_five_record_programs_are_bounded_and_use_pdf_numbers() {
        let records: [PdfRadialRecord; 25] = std::array::from_fn(|index| {
            let n = index as f32;
            [
                0.1 + n * 0.03,
                0.5,
                0.02,
                0.1,
                0.08 + n * 0.001,
                0.9,
                (index % 3 == 0) as u8 as f32 * 0.8,
                (index % 3 == 1) as u8 as f32 * 0.6,
                (index % 3 == 2) as u8 as f32 * 0.7,
            ]
        });
        for mode in [
            PdfRadialCoverage::DirectSourceOver,
            PdfRadialCoverage::EnhancedMax,
        ] {
            let program = compile(&records, bounds(), 128.0 / 255.0, mode).unwrap();
            assert!(program.rgb.len() + program.mask.len() < 64 * 1024);
            let rgb = std::str::from_utf8(&program.rgb).unwrap();
            let mask = std::str::from_utf8(&program.mask).unwrap();
            assert!(calculator_numbers_are_fixed(rgb));
            assert!(calculator_numbers_are_fixed(mask));
            let sample = evaluate(&records, 128.0 / 255.0, mode, 0.5, 0.5);
            assert!(sample.into_iter().all(f64::is_finite));
            assert!((0.0..=1.0).contains(&sample[3]));
        }
    }

    fn calculator_numbers_are_fixed(code: &str) -> bool {
        code.split_whitespace()
            .filter(|token| token.parse::<f64>().is_ok())
            .all(|token| !token.contains('e') && !token.contains('E'))
    }

    #[test]
    fn rejects_nonfinite_out_of_range_and_unbounded_inputs() {
        let mut records = overlapping();
        records[0][0] = f32::NAN;
        assert!(compile(&records, bounds(), 1.0, PdfRadialCoverage::EnhancedMax).is_err());
        let mut records = overlapping();
        records[0][6] = 1.5;
        assert!(compile(&records, bounds(), 1.0, PdfRadialCoverage::EnhancedMax).is_err());
        let mut records = overlapping();
        records[0][4] = 0.5;
        assert!(compile(&records, bounds(), 1.0, PdfRadialCoverage::EnhancedMax).is_err());
        let records = vec![overlapping()[0]; MAX_RECORDS + 1];
        assert!(compile(&records, bounds(), 1.0, PdfRadialCoverage::EnhancedMax).is_err());
    }

    #[test]
    fn fixed_decimal_formatter_expands_small_and_large_numbers() {
        for value in [1.0e-30, -2.5e-9, 0.125, 1.0, -0.0] {
            let formatted = pdf_number(value);
            assert!(!formatted.contains('e') && !formatted.contains('E'));
            let parsed = formatted.parse::<f64>().unwrap();
            assert!((parsed - value).abs() <= value.abs().max(1.0) * 1e-16);
        }
    }

    #[test]
    fn pdf_local_y_is_flipped_for_the_svg_marker_coordinate_frame() {
        let record = [[0.31, 0.2, 0.04, 0.05, 0.07, 0.9, 0.1, 0.3, 0.6]];
        let program = compile(&record, bounds(), 1.0, PdfRadialCoverage::EnhancedMax).unwrap();
        let expected_y = pdf_number(1.0 - f64::from(record[0][1]));
        assert!(
            std::str::from_utf8(&program.rgb)
                .unwrap()
                .contains(&expected_y)
        );
    }

    #[test]
    fn generated_type4_programs_match_cpu_law_and_keep_constant_stack() {
        let records = [
            [0.31, 0.2, 0.04, 0.05, 0.07, 0.9, 1.0, 0.0, 0.0],
            [0.34, 0.23, 0.05, 0.04, 0.02, 0.6, 0.0, 0.0, 1.0],
        ];
        let alpha = 128.0 / 255.0;
        let bounds = [0.0, 0.0, 1.0, 1.0];
        let pdf_x = 0.33;
        let source_y = 0.22;
        let pdf_y = 1.0 - source_y;

        for order in [records, [records[1], records[0]]] {
            for mode in [
                PdfRadialCoverage::DirectSourceOver,
                PdfRadialCoverage::EnhancedMax,
            ] {
                let program = compile(&order, bounds, alpha, mode).unwrap();
                let (rgb, rgb_stack) = run_calculator(&program.rgb, pdf_x, pdf_y);
                let (mask, mask_stack) = run_calculator(&program.mask, pdf_x, pdf_y);
                assert_eq!(rgb.len(), 3);
                assert_eq!(
                    mask.len(),
                    1,
                    "mode {mode:?}, mask {mask:?}, code {}",
                    std::str::from_utf8(&program.mask).unwrap()
                );
                let expected = evaluate(&order, alpha, mode, pdf_x, source_y);
                for (actual, expected) in rgb.iter().zip(expected) {
                    assert!((*actual - expected).abs() < 1e-12, "{actual} != {expected}");
                }
                let expected_mask = match mode {
                    PdfRadialCoverage::DirectSourceOver => expected[3],
                    PdfRadialCoverage::EnhancedMax => expected[3] / f64::from(alpha),
                };
                assert!((mask[0] - expected_mask).abs() < 1e-12);
                assert!(rgb_stack <= 20, "RGB stack grew to {rgb_stack}");
                assert!(mask_stack <= 8, "mask stack grew to {mask_stack}");
            }
        }

        // Exercise both conditional zero-q paths and a 25-record program. The
        // latter confirms the calculator stack is bounded independently of N.
        let zero = [[0.5, 0.5, 0.1, 0.1, 0.0, 0.0, 0.8, 0.3, 0.1]];
        let program = compile(&zero, bounds, alpha, PdfRadialCoverage::DirectSourceOver).unwrap();
        let (rgb, rgb_stack) = run_calculator(&program.rgb, 0.9, 0.9);
        let (mask, mask_stack) = run_calculator(&program.mask, 0.9, 0.9);
        assert_eq!(rgb, vec![0.0; 3]);
        assert_eq!(mask, vec![0.0]);
        assert!(rgb_stack <= 20);
        assert!(mask_stack <= 8);

        let many: [PdfRadialRecord; 25] = std::array::from_fn(|index| {
            let n = index as f32;
            [0.1 + n * 0.03, 0.5, 0.02, 0.1, 0.08, 0.9, 0.8, 0.3, 0.1]
        });
        let program = compile(&many, bounds, alpha, PdfRadialCoverage::EnhancedMax).unwrap();
        let (_, rgb_stack) = run_calculator(&program.rgb, 0.5, 0.5);
        let (_, mask_stack) = run_calculator(&program.mask, 0.5, 0.5);
        assert!(rgb_stack <= 20, "RGB stack grew to {rgb_stack}");
        assert!(mask_stack <= 8, "mask stack grew to {mask_stack}");
    }
}

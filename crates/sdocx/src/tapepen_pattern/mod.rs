//! Experimental restricted TapePen pattern asset rasterizer.
//! Accepts the observed serialized pattern structure, not arbitrary SVG.
mod scan;
mod stroke;
mod texture;

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum AssetError {
    #[error("unsupported TapePen SVG document")]
    UnsupportedDocument,
    #[error("invalid TapePen SVG value")]
    InvalidValue,
    #[error("TapePen pattern size must be 64 or 128")]
    UnsupportedSize,
    #[error("TapePen pattern geometry exceeds supported bounds")]
    Geometry,
}

fn color(s: &str) -> Result<u32, AssetError> {
    if s.len() != 7 || !s.starts_with('#') || !s.is_ascii() {
        return Err(AssetError::InvalidValue);
    }
    let rgb = u32::from_str_radix(&s[1..], 16).map_err(|_| AssetError::InvalidValue)?;
    Ok(0xff000000 | ((rgb & 255) << 16) | (rgb & 0xff00) | (rgb >> 16))
}
fn attribute<'a>(s: &'a str, prefix: &str, suffix: &str) -> Result<(&'a str, &'a str), AssetError> {
    s.strip_prefix(prefix)
        .and_then(|s| s.split_once(suffix))
        .ok_or(AssetError::UnsupportedDocument)
}
fn number(s: &str) -> Result<f32, AssetError> {
    let n: f32 = s.parse().map_err(|_| AssetError::InvalidValue)?;
    if !n.is_finite() {
        return Err(AssetError::InvalidValue);
    }
    Ok(n)
}
fn point(s: &str) -> Result<(f32, f32), AssetError> {
    let (x, y) = s.split_once(' ').ok_or(AssetError::UnsupportedDocument)?;
    Ok((number(x)?, number(y)?))
}
fn expect_line<'a>(
    lines: &mut impl Iterator<Item = &'a str>,
    expected: &str,
) -> Result<(), AssetError> {
    if lines.next() != Some(expected) {
        return Err(AssetError::UnsupportedDocument);
    }
    Ok(())
}
/// Returns straight RGBA8 pixels. This deliberately rejects unverified input forms.
pub fn rasterize(source: &str, size: usize) -> Result<Vec<u8>, AssetError> {
    if size != 64 && size != 128 {
        return Err(AssetError::UnsupportedSize);
    }
    if source.len() > 100_000 {
        return Err(AssetError::UnsupportedDocument);
    }
    let mut lines = source.lines();
    expect_line(
        &mut lines,
        "<svg width=\"120\" height=\"120\" viewBox=\"0 0 120 120\" fill=\"none\" xmlns=\"http://www.w3.org/2000/svg\">",
    )?;
    expect_line(&mut lines, "<g clip-path=\"url(#clip0_17692_100364)\">")?;
    let mut image = vec![0; size * size * 4];
    let full = vec![255; size * size];
    let rect = |line: &str| -> Result<u32, AssetError> {
        let (c, end) = attribute(line, "<rect width=\"120\" height=\"120\" fill=\"", "\"/>")?;
        if !end.is_empty() {
            return Err(AssetError::UnsupportedDocument);
        }
        color(c)
    };
    texture::blend(
        &mut image,
        &full,
        rect(lines.next().ok_or(AssetError::UnsupportedDocument)?)?,
        1.0,
    );
    expect_line(&mut lines, "<g clip-path=\"url(#clip1_17692_100364)\">")?;
    texture::blend(
        &mut image,
        &full,
        rect(lines.next().ok_or(AssetError::UnsupportedDocument)?)?,
        1.0,
    );
    let mut count = 0;
    loop {
        let line = lines.next().ok_or(AssetError::UnsupportedDocument)?;
        if line == "</g>" {
            break;
        }
        if count == 136 {
            return Err(AssetError::UnsupportedDocument);
        }
        let (d, rest) = attribute(line, "<path d=\"M", "\" stroke=\"")?;
        let (c, rest) = rest
            .split_once("\" stroke-width=\"")
            .ok_or(AssetError::UnsupportedDocument)?;
        let (w, rest) = rest
            .split_once("\" stroke-linecap=\"round\"/>")
            .ok_or(AssetError::UnsupportedDocument)?;
        if !rest.is_empty() {
            return Err(AssetError::UnsupportedDocument);
        }
        let (start, end) = if let Some((a, b)) = d.split_once('V') {
            let (x, y) = point(a)?;
            ((x, y), (x, number(b)?))
        } else {
            let (a, b) = d.split_once('L').ok_or(AssetError::UnsupportedDocument)?;
            (point(a)?, point(b)?)
        };
        let dx = end.0 - start.0;
        let dy = end.1 - start.1;
        let p = [
            start.0,
            start.1,
            start.0 + dx / 3.0,
            start.1 + dy / 3.0,
            end.0 - dx / 3.0,
            end.1 - dy / 3.0,
            end.0,
            end.1,
        ];
        let paint = color(c)?;
        let scale = size as f32 / 120.0;
        if !p
            .iter()
            .all(|v| v.is_finite() && (v * scale).abs() <= 1000.0)
            || (dx * dx + dy * dy).sqrt() * scale <= 0.01
        {
            return Err(AssetError::Geometry);
        }
        let width = number(w)?;
        if width < 0.0 {
            return Err(AssetError::InvalidValue);
        }
        count += 1;
        if width * scale <= 0.01 {
            continue;
        }
        let edges = stroke::try_stroke(p, width, scale).map_err(|_| AssetError::Geometry)?;
        texture::blend(
            &mut image,
            &scan::try_coverage(&edges, size, size).map_err(|_| AssetError::Geometry)?,
            paint,
            1.0,
        );
    }
    if count != 136 {
        return Err(AssetError::UnsupportedDocument);
    }
    for expected in [
        "</g>",
        "<defs>",
        "<clipPath id=\"clip0_17692_100364\">",
        "<rect width=\"120\" height=\"120\" fill=\"white\"/>",
        "</clipPath>",
        "<clipPath id=\"clip1_17692_100364\">",
        "<rect width=\"120\" height=\"120\" fill=\"white\"/>",
        "</clipPath>",
        "</defs>",
        "</svg>",
    ] {
        expect_line(&mut lines, expected)?;
    }
    if lines.next().is_some() {
        return Err(AssetError::UnsupportedDocument);
    }
    texture::finish(&mut image, size, size);
    Ok(image)
}

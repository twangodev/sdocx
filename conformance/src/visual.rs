use std::collections::BTreeSet;
use std::fs;
use std::io::Read;
use std::path::{Component, Path, PathBuf};
use std::process::{Command, Output};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::Result;
use crate::image::{MetricsOptions, PixelMetrics, RgbImage, measure};

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    pub version: u32,
    pub fixtures: Vec<FixtureEntry>,
}
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FixtureEntry {
    pub id: String,
    pub sdocx: Asset,
    pub reference_pdf: Asset,
    pub expected: Expected,
}
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Asset {
    pub path: String,
    pub sha256: String,
}
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Expected {
    pub stored_pages: u64,
    pub visible_pages: u64,
    #[serde(default)]
    pub title: Option<serde_json::Value>,
    #[serde(default)]
    pub body: Option<serde_json::Value>,
    #[serde(default)]
    pub flow: Option<serde_json::Value>,
    #[serde(default)]
    pub page_objects: Option<serde_json::Value>,
    #[serde(default)]
    pub diagnostics: Option<serde_json::Value>,
}
#[derive(Clone, Debug)]
pub struct Fixture {
    pub id: String,
    pub sdocx: PathBuf,
    pub reference_pdf: PathBuf,
    pub sdocx_sha256: String,
    pub reference_pdf_sha256: String,
    pub visible_pages: usize,
}

pub fn sha256(path: impl AsRef<Path>) -> Result<String> {
    let mut input = fs::File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 64 * 1024];
    loop {
        let n = input.read(&mut buffer)?;
        if n == 0 {
            break;
        }
        hasher.update(&buffer[..n]);
    }
    let digest = hasher.finalize();
    Ok(digest.iter().map(|b| format!("{b:02x}")).collect())
}
pub fn read_fixtures(
    manifest_path: &Path,
    corpus: &Path,
    selected: &[String],
) -> Result<Vec<Fixture>> {
    let raw = fs::read(manifest_path)?;
    let doc: Manifest = serde_json::from_slice(&raw)?;
    if doc.version != 1 || doc.fixtures.is_empty() {
        return Err("manifest must use version 1 and contain fixtures".into());
    }
    let corpus = corpus.canonicalize()?;
    let selection: BTreeSet<_> = selected.iter().cloned().collect();
    let mut seen = BTreeSet::new();
    let mut out = Vec::new();
    for e in doc.fixtures {
        if e.id.is_empty()
            || !e
                .id
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || b"_-".contains(&c))
            || !seen.insert(e.id.clone())
        {
            return Err(format!("invalid or duplicate fixture ID: {}", e.id).into());
        }
        if e.expected.visible_pages == 0 || e.expected.visible_pages > e.expected.stored_pages {
            return Err(format!("{}: invalid page counts", e.id).into());
        }
        for asset in [&e.sdocx, &e.reference_pdf] {
            let p = Path::new(&asset.path);
            if asset.path.is_empty()
                || asset.path.contains('\\')
                || asset.path.contains(':')
                || p.is_absolute()
                || p.components().any(|c| c == Component::ParentDir)
            {
                return Err(format!(
                    "{}: file is outside the corpus or has an invalid path",
                    e.id
                )
                .into());
            }
            if asset.sha256.len() != 64
                || !asset
                    .sha256
                    .bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
            {
                return Err(format!("{}: invalid SHA-256", e.id).into());
            }
        }
        if !selection.is_empty() && !selection.contains(&e.id) {
            continue;
        }
        let resolve = |asset: &Asset| -> Result<PathBuf> {
            let p = corpus.join(&asset.path).canonicalize()?;
            if !p.starts_with(&corpus) {
                return Err(format!("{}: file is outside the corpus", e.id).into());
            }
            if sha256(&p)? != asset.sha256 {
                return Err(format!("{}: SHA-256 mismatch for {}", e.id, asset.path).into());
            }
            Ok(p)
        };
        let sdocx = resolve(&e.sdocx)?;
        let reference_pdf = resolve(&e.reference_pdf)?;
        out.push(Fixture {
            id: e.id,
            sdocx,
            reference_pdf,
            sdocx_sha256: e.sdocx.sha256,
            reference_pdf_sha256: e.reference_pdf.sha256,
            visible_pages: e.expected.visible_pages as usize,
        });
    }
    if let Some(id) = selection.difference(&seen).next() {
        return Err(format!("unknown fixture ID: {id}").into());
    }
    if out.is_empty() {
        return Err("no fixtures selected".into());
    }
    Ok(out)
}

#[derive(Clone, Debug, Serialize)]
struct PageReport {
    page: usize,
    size: [u32; 2],
    pdf_size_points: [f64; 2],
    metrics: PixelMetrics,
    reference: String,
    sdk: String,
    difference: String,
    reference_png_sha256: String,
    sdk_png_sha256: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    sdk_pdf_size_points: Option<[f64; 2]>,
    #[serde(skip_serializing_if = "Option::is_none")]
    sdk_text_characters: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    sdk_text: Option<String>,
}
#[derive(Clone, Debug, Serialize)]
struct FixtureReport {
    id: String,
    sdk_format: String,
    sdocx_sha256: String,
    reference_pdf_sha256: String,
    pages: Vec<PageReport>,
    #[serde(skip_serializing_if = "Option::is_none")]
    sdk_pdf: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    sdk_pdf_sha256: Option<String>,
}
#[derive(Clone, Debug, Serialize)]
struct Report {
    schema_version: u32,
    options: MetricsOptionsJson,
    environment: serde_json::Value,
    fixtures: Vec<FixtureReport>,
}
#[derive(Clone, Debug, Serialize)]
struct MetricsOptionsJson {
    channel_threshold: u8,
    ink_threshold: u8,
    tolerance: u8,
}

#[derive(Clone, Debug)]
pub struct VisualConfig {
    pub cli: PathBuf,
    pub output: PathBuf,
    pub format: String,
    pub fonts: Vec<PathBuf>,
    pub options: MetricsOptions,
    pub python: PathBuf,
}

fn command(command: &mut Command) -> Result<Output> {
    let out = command.output()?;
    if !out.status.success() {
        return Err(format!(
            "command failed ({}): {}",
            out.status,
            String::from_utf8_lossy(&out.stderr)
        )
        .into());
    }
    Ok(out)
}
fn run_pdf_python(
    python: &Path,
    pdf: &Path,
    page: usize,
    width: u32,
    out: &Path,
) -> Result<(f64, f64, String)> {
    // This inline adapter only asks PyMuPDF for PDF metadata and raster pixels. Rust owns all validation and comparisons.
    const SHIM: &str = "import json,sys,pymupdf; d=pymupdf.open(sys.argv[1]); p=d[int(sys.argv[2])]; r=p.rect; s=int(sys.argv[3])/r.width; pix=p.get_pixmap(matrix=pymupdf.Matrix(s,s),colorspace=pymupdf.csRGB,alpha=False); pix.save(sys.argv[4]); print(json.dumps({'width':r.width,'height':r.height,'version':pymupdf.__version__,'python':sys.version.split()[0],'raster_width':pix.width,'raster_height':pix.height}))";
    let stdout = command(
        Command::new(python)
            .arg("-c")
            .arg(SHIM)
            .arg(pdf)
            .arg(page.to_string())
            .arg(width.to_string())
            .arg(out),
    )?
    .stdout;
    let info: serde_json::Value = serde_json::from_slice(&stdout)?;
    Ok((
        info["width"].as_f64().ok_or("bad PDF width")?,
        info["height"].as_f64().ok_or("bad PDF height")?,
        info["version"].as_str().unwrap_or("unknown").to_owned(),
    ))
}
fn raster_pdf_page(
    python: &Path,
    pdf: &Path,
    index: usize,
    size: (u32, u32),
    out: &Path,
) -> Result<(RgbImage, [f64; 2], String)> {
    let (pw, ph, version) = run_pdf_python(python, pdf, index, size.0, out)?;
    let raster = RgbImage::from_png(out)?;
    let image = normalize_raster(raster, size, [pw, ph])?;
    Ok((image, [pw, ph], version))
}

fn normalize_raster(raster: RgbImage, size: (u32, u32), points: [f64; 2]) -> Result<RgbImage> {
    let (pw, ph) = (points[0], points[1]);
    let expected_h = size.0 as f64 * ph / pw;
    if pw <= 0.0
        || ph <= 0.0
        || (size.1 as f64 - expected_h).abs() > (1.0f64).max(0.5 * size.0 as f64 / pw)
    {
        return Err(format!(
            "page aspect ratio mismatch: requested {}x{}, PDF {pw}x{ph}",
            size.0, size.1
        )
        .into());
    }
    if raster.width < size.0
        || raster.width > size.0 + 1
        || raster.height < expected_h.floor() as u32
        || raster.height > expected_h.ceil() as u32 + 1
    {
        return Err("unexpected PDF raster dimensions".into());
    }
    let mut canvas = RgbImage::blank(size.0, size.1, [255; 3]);
    for y in 0..raster.height.min(size.1) {
        for x in 0..raster.width.min(size.0) {
            canvas.set_pixel(x, y, raster.pixel(x, y));
        }
    }
    Ok(canvas)
}
fn candidate_paths(dir: &Path, count: usize) -> Result<Vec<PathBuf>> {
    let paths: Vec<_> = if count == 1 {
        vec![dir.join("sdk.png")]
    } else {
        (0..count)
            .map(|i| dir.join(format!("sdk_page{i}.png")))
            .collect()
    };
    let actual: BTreeSet<_> = fs::read_dir(dir)?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|s| s.starts_with("sdk") && s.ends_with(".png"))
        })
        .collect();
    if actual != paths.iter().cloned().collect() {
        return Err(format!(
            "{}: SDK page count/names differ from manifest",
            dir.display()
        )
        .into());
    }
    Ok(paths)
}
fn git(root: &Path, args: &[&str]) -> Result<String> {
    let output = command(Command::new("git").args(args).current_dir(root))?;
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_owned())
}
fn font_inventory() -> Result<serde_json::Value> {
    let out = match Command::new("fc-list").arg("--format=%{file}\\n").output() {
        Ok(out) => out,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(serde_json::json!({"available":false,"reason":"fc-list is unavailable"}));
        }
        Err(error) => return Err(error.into()),
    };
    if !out.status.success() {
        return Err(format!("fc-list failed: {}", String::from_utf8_lossy(&out.stderr)).into());
    }
    let paths: BTreeSet<_> = String::from_utf8_lossy(&out.stdout)
        .lines()
        .map(str::to_owned)
        .collect();
    font_inventory_for_paths(paths)
}
fn font_inventory_for_paths(paths: BTreeSet<String>) -> Result<serde_json::Value> {
    let fonts: Vec<_> = paths
        .iter()
        .map(|p| Ok(serde_json::json!({"file":p,"sha256":sha256(p)?})))
        .collect::<Result<_>>()?;
    let mut hashes: Vec<_> = fonts.iter().filter_map(|x| x["sha256"].as_str()).collect();
    hashes.sort();
    Ok(
        serde_json::json!({"available":true,"fonts":fonts,"sha256":sha256_bytes(hashes.join("\n").as_bytes())}),
    )
}
fn sha256_bytes(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}
fn pdf_page_count(path: &Path) -> Result<usize> {
    Ok(lopdf::Document::load(path)?.get_pages().len())
}
fn pdf_pixel_size(points: [f64; 2]) -> (u32, u32) {
    (
        (points[0] * 96.0 / 72.0).round_ties_even() as u32,
        (points[1] * 96.0 / 72.0).round_ties_even() as u32,
    )
}

pub fn run_visual(
    fixtures: &[Fixture],
    config: &VisualConfig,
    root: &Path,
) -> Result<serde_json::Value> {
    if !matches!(config.format.as_str(), "png" | "pdf") {
        return Err("SDK output format must be png or pdf".into());
    }
    if config.output.exists() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::AlreadyExists,
            "output directory already exists",
        )
        .into());
    }
    let explicit_fonts: Vec<_> = config
        .fonts
        .iter()
        .map(|path| {
            Ok(serde_json::json!({"file":path.display().to_string(),"sha256":sha256(path)?}))
        })
        .collect::<Result<_>>()?;
    let cli = config.cli.canonicalize()?;
    let sdk_version =
        String::from_utf8_lossy(&command(Command::new(&cli).arg("--version"))?.stdout)
            .trim()
            .to_owned();
    let sdk_binary_sha256 = sha256(&cli)?;
    let workspace_git_head = git(root, &["rev-parse", "HEAD"])?;
    let dirty = !git(root, &["status", "--porcelain"])?.is_empty();
    let fonts = font_inventory()?;
    let python_environment = python_environment(&config.python)?;
    fs::create_dir(&config.output)?;
    let mut reports = Vec::new();
    let mut pymupdf_version = String::new();
    for fixture in fixtures {
        let dir = config.output.join(&fixture.id);
        fs::create_dir(&dir)?;
        if pdf_page_count(&fixture.reference_pdf)? != fixture.visible_pages {
            return Err(format!("{}: PDF page count differs from manifest", fixture.id).into());
        }
        let sdk_output = dir.join(format!("sdk.{}", config.format));
        let mut cmd = Command::new(&cli);
        cmd.arg(&fixture.sdocx).arg("--output").arg(&sdk_output);
        for font in &config.fonts {
            cmd.arg("--font").arg(font);
        }
        let conv = command(&mut cmd)?;
        fs::write(dir.join("sdk.log"), [conv.stdout, conv.stderr].concat())?;
        let candidates = if config.format == "png" {
            candidate_paths(&dir, fixture.visible_pages)?
        } else {
            if pdf_page_count(&sdk_output)? != fixture.visible_pages {
                return Err("SDK PDF page count differs from manifest".into());
            }
            (0..fixture.visible_pages)
                .map(|i| {
                    dir.join(if fixture.visible_pages == 1 {
                        "sdk.png".to_owned()
                    } else {
                        format!("sdk_page{i}.png")
                    })
                })
                .collect()
        };
        let mut pages = Vec::new();
        for (i, candidate_path) in candidates.iter().enumerate() {
            let candidate = if config.format == "png" {
                RgbImage::from_png(candidate_path)?
            } else {
                let info = pdf_page_info(&config.python, &sdk_output, i)?;
                pymupdf_version = info.version;
                let size = pdf_pixel_size(info.points);
                let raster_path = dir.join(format!("sdk_render_raw_{i}.png"));
                let (image, _, version) =
                    raster_pdf_page(&config.python, &sdk_output, i, size, &raster_path)?;
                if !version.is_empty() {
                    pymupdf_version = version;
                }
                let dest = candidate_path;
                image.to_png(dest)?;
                let _ = fs::remove_file(raster_path);
                image
            };
            let reference_raw = dir.join(format!("reference_raw_{i}.png"));
            let (reference, pdf_points, version) = raster_pdf_page(
                &config.python,
                &fixture.reference_pdf,
                i,
                (candidate.width, candidate.height),
                &reference_raw,
            )?;
            fs::remove_file(reference_raw)?;
            if !version.is_empty() {
                pymupdf_version = version;
            }
            let (metrics, diff) = measure(&reference, &candidate, config.options)?;
            let ref_path = dir.join(format!("reference_page{i}.png"));
            let diff_path = dir.join(format!("difference_page{i}.png"));
            reference.to_png(&ref_path)?;
            diff.to_png(&diff_path)?;
            let (sdk_pdf_size_points, sdk_text_characters, sdk_text) = if config.format == "pdf" {
                let info = pdf_page_info(&config.python, &sdk_output, i)?;
                pymupdf_version = info.version;
                let text = info.text;
                let text_path = dir.join(format!("sdk_page{i}.txt"));
                fs::write(&text_path, &text)?;
                (
                    Some(info.points),
                    Some(text.chars().count()),
                    Some(
                        text_path
                            .strip_prefix(&config.output)?
                            .to_string_lossy()
                            .replace('\\', "/"),
                    ),
                )
            } else {
                (None, None, None)
            };
            pages.push(PageReport {
                page: i + 1,
                size: [candidate.width, candidate.height],
                pdf_size_points: pdf_points,
                metrics,
                reference: relative(&ref_path, &config.output)?,
                sdk: relative(&candidates[i], &config.output)?,
                difference: relative(&diff_path, &config.output)?,
                reference_png_sha256: sha256(&ref_path)?,
                sdk_png_sha256: sha256(&candidates[i])?,
                sdk_pdf_size_points,
                sdk_text_characters,
                sdk_text,
            });
        }
        reports.push(FixtureReport {
            id: fixture.id.clone(),
            sdk_format: config.format.clone(),
            sdocx_sha256: fixture.sdocx_sha256.clone(),
            reference_pdf_sha256: fixture.reference_pdf_sha256.clone(),
            pages,
            sdk_pdf: if config.format == "pdf" {
                Some(relative(&sdk_output, &config.output)?)
            } else {
                None
            },
            sdk_pdf_sha256: if config.format == "pdf" {
                Some(sha256(&sdk_output)?)
            } else {
                None
            },
        });
    }
    let report = Report {
        schema_version: 1,
        options: MetricsOptionsJson {
            channel_threshold: config.options.channel_threshold,
            ink_threshold: config.options.ink_threshold,
            tolerance: config.options.tolerance,
        },
        environment: serde_json::json!({"python":python_environment.version,"platform":python_environment.platform,"pymupdf":pymupdf_version,"sdk_version":sdk_version,"sdk_binary_sha256":sdk_binary_sha256,"workspace_git_head":workspace_git_head,"workspace_dirty":dirty,"fontconfig":fonts,"explicit_fonts":explicit_fonts}),
        fixtures: reports,
    };
    let value = serde_json::to_value(&report)?;
    fs::write(
        config.output.join("report.json"),
        format!("{}\n", serde_json::to_string_pretty(&value)?),
    )?;
    fs::write(config.output.join("index.html"), write_html(&value))?;
    for f in value["fixtures"].as_array().unwrap() {
        for p in f["pages"].as_array().unwrap() {
            eprintln!(
                "{} page {}: changed {:.2}%, missing ink {:.2}%, extra ink {:.2}%",
                f["id"],
                p["page"],
                p["metrics"]["changed_pixel_fraction"]
                    .as_f64()
                    .unwrap_or(0.0)
                    * 100.0,
                p["metrics"]["missing_ink_fraction"].as_f64().unwrap_or(0.0) * 100.0,
                p["metrics"]["extra_ink_fraction"].as_f64().unwrap_or(0.0) * 100.0
            );
        }
    }
    println!("Report: {}", config.output.join("index.html").display());
    Ok(value)
}
fn relative(path: &Path, base: &Path) -> Result<String> {
    Ok(path
        .strip_prefix(base)?
        .to_string_lossy()
        .replace('\\', "/"))
}
struct PythonEnvironment {
    version: String,
    platform: String,
}
fn python_environment(python: &Path) -> Result<PythonEnvironment> {
    const CODE: &str = "import json,platform,sys; print(json.dumps({'version':sys.version.split()[0],'platform':platform.platform()}))";
    let out = command(Command::new(python).arg("-c").arg(CODE))?;
    let value: serde_json::Value = serde_json::from_slice(&out.stdout)?;
    Ok(PythonEnvironment {
        version: value["version"]
            .as_str()
            .ok_or("missing Python version")?
            .to_owned(),
        platform: value["platform"]
            .as_str()
            .ok_or("missing Python platform")?
            .to_owned(),
    })
}
struct PdfPageInfo {
    points: [f64; 2],
    text: String,
    version: String,
}
fn pdf_page_info(python: &Path, pdf: &Path, page: usize) -> Result<PdfPageInfo> {
    const CODE: &str = "import json,sys,pymupdf; p=pymupdf.open(sys.argv[1])[int(sys.argv[2])]; r=p.rect; print(json.dumps({'width':r.width,'height':r.height,'text':p.get_text(),'version':pymupdf.__version__}))";
    let out = command(
        Command::new(python)
            .arg("-c")
            .arg(CODE)
            .arg(pdf)
            .arg(page.to_string()),
    )?;
    let v: serde_json::Value = serde_json::from_slice(&out.stdout)?;
    Ok(PdfPageInfo {
        points: [
            v["width"].as_f64().ok_or("bad width")?,
            v["height"].as_f64().ok_or("bad height")?,
        ],
        text: v["text"]
            .as_str()
            .ok_or("missing PDF page text")?
            .to_owned(),
        version: v["version"]
            .as_str()
            .ok_or("missing PyMuPDF version")?
            .to_owned(),
    })
}
pub fn write_html(report: &serde_json::Value) -> String {
    let mut sections = String::new();
    for fixture in report["fixtures"].as_array().into_iter().flatten() {
        let id = fixture["id"].as_str().unwrap_or("");
        sections.push_str(&format!("<h2>{}</h2>", escape(id)));
        if let Some(pdf) = fixture["sdk_pdf"].as_str() {
            sections.push_str(&format!(
                "<p><a href=\"{}\">Download SDK PDF</a></p>",
                escape_attr(pdf)
            ));
        }
        for page in fixture["pages"].as_array().into_iter().flatten() {
            let num = page["page"].as_u64().unwrap_or(0);
            let get = |k: &str| page[k].as_str().unwrap_or("");
            let m = &page["metrics"];
            let changed = m["changed_pixel_fraction"].as_f64().unwrap_or(0.0) * 100.0;
            let missing = m["missing_ink_fraction"].as_f64().unwrap_or(0.0) * 100.0;
            let extra = m["extra_ink_fraction"].as_f64().unwrap_or(0.0) * 100.0;
            let r = escape_attr(get("reference"));
            let s = escape_attr(get("sdk"));
            let d = escape_attr(get("difference"));
            sections.push_str(&format!("<details open><summary>Page {num} · changed {changed:.2}% · missing ink {missing:.2}% · extra ink {extra:.2}%</summary><div class=\"pair\"><figure><figcaption>Samsung PDF</figcaption><a href=\"{r}\"><img src=\"{r}\" alt=\"Samsung reference page\"></a></figure><figure><figcaption>SDK</figcaption><a href=\"{s}\"><img src=\"{s}\" alt=\"SDK page\"></a></figure></div><details><summary>Overlay and difference</summary><label>SDK overlay opacity <input type=\"range\" min=\"0\" max=\"100\" value=\"50\"></label><div class=\"pair\"><div class=\"overlay\"><img src=\"{r}\"><img class=\"candidate\" src=\"{s}\"></div><figure><figcaption>Absolute RGB difference ×4 (black = identical)</figcaption><a href=\"{d}\"><img src=\"{d}\"></a></figure></div></details></details>"));
        }
    }
    format!(
        "<!doctype html><html lang=\"en\"><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width,initial-scale=1\"><title>SDOCX visual comparison</title><style>body{{font:16px system-ui,sans-serif;max-width:1500px;margin:32px auto;padding:0 24px;color:#252525;background:#f6f6f6}}.pair{{display:grid;grid-template-columns:1fr 1fr;gap:16px;margin:12px 0 24px}}figure{{margin:0}}img{{display:block;width:100%;height:auto}}summary{{cursor:pointer;padding:12px 0}}.overlay{{position:relative;align-self:start}}.candidate{{position:absolute;inset:0;opacity:.5}}label{{display:block;margin:12px 0}}</style><h1>SDOCX visual comparison</h1><p>Samsung reference exports compared with SDK output in visible page order. Page sizes are normalized without shifting or aligning content. Pixel scores include font and antialiasing differences; measurements are not a compatibility verdict. Ink metrics assume a white or near-white canvas. See <a href=\"report.json\">report.json</a> for hashes, settings, tool versions, and font inventory.</p>{sections}<script>document.querySelectorAll('input[type=range]').forEach(i=>i.addEventListener('input',()=>i.closest('details').querySelector('.candidate').style.opacity=i.value/100));</script></html>"
    )
}
fn escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace("'", "&#39;")
}
fn escape_attr(s: &str) -> String {
    escape(s)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::image::{MetricsOptions, RgbImage};
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp() -> PathBuf {
        std::env::temp_dir().join(format!(
            "sdocx-conformance-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ))
    }
    fn asset(path: &str, hash: &str) -> serde_json::Value {
        serde_json::json!({"path":path,"sha256":hash})
    }
    fn pdf_with_pages(path: &Path, count: usize) {
        use lopdf::{Dictionary, Document, Object};
        let mut pdf = Document::with_version("1.5");
        let pages_id = pdf.new_object_id();
        let page_ids: Vec<_> = (0..count).map(|_| pdf.new_object_id()).collect();
        let catalog_id = pdf.new_object_id();
        pdf.objects.insert(
            pages_id,
            Object::Dictionary(lopdf::dictionary! {
                "Type" => "Pages", "Kids" => page_ids.iter().copied().map(Object::Reference).collect::<Vec<_>>(), "Count" => count as i64
            }),
        );
        for page_id in page_ids {
            pdf.objects.insert(page_id,Object::Dictionary(lopdf::dictionary! {"Type"=>"Page","Parent"=>pages_id,"MediaBox"=>vec![0.into(),0.into(),8.into(),8.into()],"Resources"=>Dictionary::new()}));
        }
        pdf.objects.insert(
            catalog_id,
            Object::Dictionary(lopdf::dictionary! { "Type" => "Catalog", "Pages" => pages_id }),
        );
        pdf.trailer.set("Root", catalog_id);
        pdf.save(path).unwrap();
    }
    fn one_page_pdf(path: &Path) {
        pdf_with_pages(path, 1);
    }
    fn workspace_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .to_path_buf()
    }
    #[test]
    fn manifest_strict_schema_selection_hashes_and_corpus_containment() {
        let dir = temp();
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("a.sdocx"), b"a").unwrap();
        fs::write(dir.join("a.pdf"), b"b").unwrap();
        let entry = serde_json::json!({"id":"a","sdocx":asset("a.sdocx",&sha256(dir.join("a.sdocx")).unwrap()),"reference_pdf":asset("a.pdf",&sha256(dir.join("a.pdf")).unwrap()),"expected":{"stored_pages":2,"visible_pages":1}});
        let manifest = dir.join("manifest.json");
        fs::write(
            &manifest,
            serde_json::to_vec(&serde_json::json!({"version":1,"fixtures":[entry]})).unwrap(),
        )
        .unwrap();
        assert_eq!(
            read_fixtures(&manifest, &dir, &["a".into()]).unwrap().len(),
            1
        );
        assert!(
            read_fixtures(&manifest, &dir, &["missing".into()])
                .unwrap_err()
                .to_string()
                .contains("unknown fixture")
        );
        let original: serde_json::Value =
            serde_json::from_slice(&fs::read(&manifest).unwrap()).unwrap();
        let mut duplicate = original.clone();
        duplicate["fixtures"]
            .as_array_mut()
            .unwrap()
            .push(original["fixtures"][0].clone());
        fs::write(&manifest, serde_json::to_vec(&duplicate).unwrap()).unwrap();
        assert!(
            read_fixtures(&manifest, &dir, &[])
                .unwrap_err()
                .to_string()
                .contains("duplicate fixture")
        );
        let mut counts = original.clone();
        for (stored, visible) in [(0, 1), (1, 0), (1, 2)] {
            counts["fixtures"][0]["expected"]["stored_pages"] = stored.into();
            counts["fixtures"][0]["expected"]["visible_pages"] = visible.into();
            fs::write(&manifest, serde_json::to_vec(&counts).unwrap()).unwrap();
            assert!(read_fixtures(&manifest, &dir, &[]).is_err());
        }
        let mut badpath = original.clone();
        badpath["fixtures"][0]["sdocx"]["path"] = "../a.sdocx".into();
        fs::write(&manifest, serde_json::to_vec(&badpath).unwrap()).unwrap();
        assert!(
            read_fixtures(&manifest, &dir, &[])
                .unwrap_err()
                .to_string()
                .contains("outside the corpus")
        );
        fs::write(&manifest, serde_json::to_vec(&original).unwrap()).unwrap();
        fs::write(dir.join("a.sdocx"), b"tampered").unwrap();
        assert!(
            read_fixtures(&manifest, &dir, &[])
                .unwrap_err()
                .to_string()
                .contains("SHA-256 mismatch")
        );
        fs::write(dir.join("a.sdocx"), b"a").unwrap();
        let mut value: serde_json::Value =
            serde_json::from_slice(&fs::read(&manifest).unwrap()).unwrap();
        value["fixtures"][0]["expected"]["typo"] = 1.into();
        fs::write(&manifest, serde_json::to_vec(&value).unwrap()).unwrap();
        assert!(read_fixtures(&manifest, &dir, &[]).is_err());
        let _ = fs::remove_dir_all(dir);
    }
    #[test]
    fn unselected_fixture_files_are_not_opened_but_metadata_is_validated() {
        let dir = temp();
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("chosen.sdocx"), b"a").unwrap();
        fs::write(dir.join("chosen.pdf"), b"b").unwrap();
        let chosen = serde_json::json!({"id":"chosen","sdocx":asset("chosen.sdocx",&sha256(dir.join("chosen.sdocx")).unwrap()),"reference_pdf":asset("chosen.pdf",&sha256(dir.join("chosen.pdf")).unwrap()),"expected":{"stored_pages":1,"visible_pages":1}});
        let absent = serde_json::json!({"id":"absent","sdocx":asset("missing.sdocx",&"0".repeat(64)),"reference_pdf":asset("missing.pdf",&"1".repeat(64)),"expected":{"stored_pages":2,"visible_pages":1}});
        let manifest = dir.join("manifest.json");
        fs::write(
            &manifest,
            serde_json::to_vec(&serde_json::json!({"version":1,"fixtures":[chosen,absent]}))
                .unwrap(),
        )
        .unwrap();
        assert_eq!(
            read_fixtures(&manifest, &dir, &["chosen".into()])
                .unwrap()
                .len(),
            1
        );
        let mut invalid: serde_json::Value =
            serde_json::from_slice(&fs::read(&manifest).unwrap()).unwrap();
        invalid["fixtures"][1]["sdocx"]["sha256"] = "bad".into();
        fs::write(&manifest, serde_json::to_vec(&invalid).unwrap()).unwrap();
        assert!(
            read_fixtures(&manifest, &dir, &["chosen".into()])
                .unwrap_err()
                .to_string()
                .contains("invalid SHA-256")
        );
        let _ = fs::remove_dir_all(dir);
    }
    #[cfg(unix)]
    #[test]
    fn symlinked_assets_cannot_escape_the_corpus() {
        use std::os::unix::fs::symlink;
        let dir = temp();
        let outside = temp();
        fs::create_dir_all(&dir).unwrap();
        fs::create_dir_all(&outside).unwrap();
        fs::write(outside.join("outside.sdocx"), b"outside").unwrap();
        fs::write(dir.join("ref.pdf"), b"ref").unwrap();
        symlink(outside.join("outside.sdocx"), dir.join("link.sdocx")).unwrap();
        let fixture = serde_json::json!({"id":"escape","sdocx":asset("link.sdocx",&sha256(outside.join("outside.sdocx")).unwrap()),"reference_pdf":asset("ref.pdf",&sha256(dir.join("ref.pdf")).unwrap()),"expected":{"stored_pages":1,"visible_pages":1}});
        let manifest = dir.join("manifest.json");
        fs::write(
            &manifest,
            serde_json::to_vec(&serde_json::json!({"version":1,"fixtures":[fixture]})).unwrap(),
        )
        .unwrap();
        assert!(
            read_fixtures(&manifest, &dir, &[])
                .unwrap_err()
                .to_string()
                .contains("outside the corpus")
        );
        let _ = fs::remove_dir_all(dir);
        let _ = fs::remove_dir_all(outside);
    }
    #[test]
    fn pages_are_numerically_named_and_reject_extra_files() {
        let dir = temp();
        fs::create_dir_all(&dir).unwrap();
        for i in (0..12).rev() {
            fs::write(dir.join(format!("sdk_page{i}.png")), []).unwrap();
        }
        let paths = candidate_paths(&dir, 12).unwrap();
        assert_eq!(paths[2].file_name().unwrap(), "sdk_page2.png");
        assert!(candidate_paths(&dir, 11).is_err());
        let _ = fs::remove_dir_all(dir);
    }
    #[test]
    fn pdf_pixel_dimensions_use_python_bankers_rounding() {
        assert_eq!(pdf_pixel_size([7.875, 8.625]), (10, 12));
        assert_eq!(pdf_pixel_size([8.625, 7.875]), (12, 10));
    }
    #[test]
    fn git_and_font_inventory_failures_are_not_silently_reported_as_clean() {
        assert!(
            git(
                Path::new("/definitely/not/a/git/checkout"),
                &["status", "--porcelain"]
            )
            .is_err()
        );
        let missing = BTreeSet::from(["/definitely/missing/font.ttf".to_owned()]);
        assert!(font_inventory_for_paths(missing).is_err());
    }
    #[test]
    fn comparison_separates_rgb_delta_from_tolerant_ink_and_checks_size() {
        let mut a = RgbImage::blank(20, 20, [255; 3]);
        a.set_pixel(5, 5, [0; 3]);
        let mut b = RgbImage::blank(20, 20, [255; 3]);
        b.set_pixel(6, 5, [0; 3]);
        let (m, d) = measure(&a, &b, MetricsOptions::default()).unwrap();
        assert_eq!(m.missing_ink_fraction, 0.0);
        assert!(m.changed_pixel_fraction > 0.0);
        assert_eq!(d.pixel(5, 5), [255; 3]);
        assert!(
            measure(
                &a,
                &RgbImage::blank(1, 1, [0; 3]),
                MetricsOptions::default()
            )
            .unwrap_err()
            .to_string()
            .contains("matching dimensions")
        );
        let (m, _) = measure(
            &RgbImage::blank(1, 1, [255; 3]),
            &RgbImage::blank(1, 1, [0; 3]),
            MetricsOptions {
                tolerance: 0,
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(m.mean_absolute_rgb_error, 1.0);
    }
    #[test]
    fn blank_sdk_output_and_unexpected_ink_are_both_reported() {
        let mut ink = RgbImage::blank(100, 100, [255; 3]);
        ink.set_pixel(10, 10, [0; 3]);
        let (missing, _) = measure(
            &ink,
            &RgbImage::blank(100, 100, [255; 3]),
            MetricsOptions::default(),
        )
        .unwrap();
        assert_eq!(missing.reference_ink_pixels, 1);
        assert_eq!(missing.sdk_ink_pixels, 0);
        assert_eq!(missing.missing_ink_fraction, 1.0);
        assert_eq!(missing.changed_pixel_fraction, 0.0001);
        let (extra, _) = measure(
            &RgbImage::blank(100, 100, [255; 3]),
            &ink,
            MetricsOptions::default(),
        )
        .unwrap();
        assert_eq!(extra.extra_ink_fraction, 1.0);
    }
    #[test]
    fn pdf_fit_uses_uniform_scale_and_white_padding_for_rounded_page_box() {
        let mut raster = RgbImage::blank(1848, 2613, [255; 3]);
        raster.set_pixel(310, 2464, [0; 3]);
        let image = normalize_raster(raster, (1848, 2613), [600.0, 848.0]).unwrap();
        assert_eq!(image.pixel(310, 2464), [0; 3]);
        assert_eq!(image.pixel(310, 2612), [255; 3]);
        assert!(
            normalize_raster(
                RgbImage::blank(1848, 2614, [255; 3]),
                (1848, 2614),
                [600.0, 848.0]
            )
            .is_err()
        );
        assert!(
            normalize_raster(
                RgbImage::blank(400, 400, [255; 3]),
                (400, 400),
                [200.0, 100.0]
            )
            .unwrap_err()
            .to_string()
            .contains("aspect ratio")
        );
        assert_eq!(
            normalize_raster(
                RgbImage::blank(400, 200, [255; 3]),
                (400, 200),
                [200.0, 100.0]
            )
            .unwrap()
            .width,
            400
        );
        assert!(
            normalize_raster(
                RgbImage::blank(400, 400, [255; 3]),
                (400, 400),
                [100.0, 200.0]
            )
            .is_err()
        );
    }
    #[test]
    fn html_escapes_fixture_ids_and_has_overlay_controls() {
        let report = serde_json::json!({"fixtures":[{"id":"<&","pages":[{"page":1,"reference":"r.png","sdk":"s.png","difference":"d.png","metrics":{"changed_pixel_fraction":0.0,"missing_ink_fraction":0.0,"extra_ink_fraction":0.0}}]}]});
        let html = write_html(&report);
        assert!(html.contains("&lt;&amp;"));
        assert!(html.contains("SDK overlay opacity"));
        assert!(html.contains("report.json"));
        let quoted = write_html(
            &serde_json::json!({"fixtures":[{"id":"ok","sdk_pdf":"a\"b.pdf","pages":[]}]}),
        );
        assert!(quoted.contains("href=\"a&quot;b.pdf\""));
    }
    #[test]
    fn existing_output_is_refused_before_resolving_or_running_cli() {
        let dir = temp();
        fs::create_dir_all(&dir).unwrap();
        let config = VisualConfig {
            cli: dir.join("missing-cli"),
            output: dir.clone(),
            format: "png".into(),
            fonts: vec![],
            options: MetricsOptions::default(),
            python: PathBuf::from("missing-python"),
        };
        let error = run_visual(&[], &config, &dir).unwrap_err();
        assert!(
            error
                .to_string()
                .contains("output directory already exists")
        );
        let _ = fs::remove_dir_all(dir);
    }
    #[test]
    fn explicit_font_hashes_are_checked_before_output_creation_or_conversion() {
        let base = temp();
        fs::create_dir_all(&base).unwrap();
        let output = base.join("result");
        let config = VisualConfig {
            cli: base.join("missing-cli"),
            output: output.clone(),
            format: "png".into(),
            fonts: vec![base.join("missing-font.ttf")],
            options: MetricsOptions::default(),
            python: base.join("missing-python"),
        };
        assert!(run_visual(&[], &config, &workspace_root()).is_err());
        assert!(!output.exists());
        let _ = fs::remove_dir_all(base);
    }
    #[cfg(unix)]
    #[test]
    fn reference_and_candidate_pdf_page_count_mismatches_fail() {
        use std::os::unix::fs::PermissionsExt;
        let base = temp();
        fs::create_dir_all(&base).unwrap();
        let source = base.join("source.sdocx");
        fs::write(&source, b"sdocx").unwrap();
        let reference = base.join("reference.pdf");
        pdf_with_pages(&reference, 2);
        let candidate = base.join("candidate.pdf");
        one_page_pdf(&candidate);
        let cli = base.join("fake-cli");
        fs::write(&cli,format!("#!/bin/sh\nif [ \"$1\" = \"--version\" ]; then echo fake; exit 0; fi\ncp '{}' \"$3\"\n",candidate.display())).unwrap();
        let mut perms = fs::metadata(&cli).unwrap().permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&cli, perms).unwrap();
        let python = base.join("python-metadata");
        fs::write(
            &python,
            "#!/bin/sh\nprintf '{\"version\":\"3.14\",\"platform\":\"test-platform\"}\\n'\n",
        )
        .unwrap();
        let mut perms = fs::metadata(&python).unwrap().permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&python, perms).unwrap();
        let fixture = Fixture {
            id: "two".into(),
            sdocx: source.clone(),
            reference_pdf: reference.clone(),
            sdocx_sha256: sha256(&source).unwrap(),
            reference_pdf_sha256: sha256(&reference).unwrap(),
            visible_pages: 1,
        };
        let first = run_visual(
            std::slice::from_ref(&fixture),
            &VisualConfig {
                cli: cli.clone(),
                output: base.join("ref-mismatch"),
                format: "pdf".into(),
                fonts: vec![],
                options: MetricsOptions::default(),
                python: python.clone(),
            },
            &workspace_root(),
        )
        .unwrap_err();
        assert!(
            first
                .to_string()
                .contains("PDF page count differs from manifest")
        );
        let fixture = Fixture {
            visible_pages: 2,
            ..fixture
        };
        let second = run_visual(
            &[fixture],
            &VisualConfig {
                cli,
                output: base.join("sdk-mismatch"),
                format: "pdf".into(),
                fonts: vec![],
                options: MetricsOptions::default(),
                python,
            },
            &workspace_root(),
        )
        .unwrap_err();
        assert!(
            second
                .to_string()
                .contains("SDK PDF page count differs from manifest")
        );
        let _ = fs::remove_dir_all(base);
    }
    #[cfg(unix)]
    #[test]
    fn successful_png_workflow_writes_hashes_metrics_log_and_relative_report_paths() {
        use std::os::unix::fs::PermissionsExt;
        let base = temp();
        fs::create_dir_all(&base).unwrap();
        let sdocx = base.join("source.sdocx");
        fs::write(&sdocx, b"archive").unwrap();
        let pdf = base.join("reference.pdf");
        pdf_with_pages(&pdf, 2);
        let source_png = base.join("page.png");
        let mut pixels = RgbImage::blank(8, 8, [255; 3]);
        pixels.set_pixel(3, 3, [0; 3]);
        pixels.to_png(&source_png).unwrap();
        let cli = base.join("fake-cli");
        let args_file = base.join("cli-args.txt");
        fs::write(&cli,format!("#!/bin/sh\nif [ \"$1\" = \"--version\" ]; then echo 'fake cli 1'; exit 0; fi\nprintf '%s\\n' \"$@\" > '{}'\noutdir=$(dirname \"$3\")\ncp '{}' \"$outdir/sdk_page0.png\"\ncp '{}' \"$outdir/sdk_page1.png\"\necho diagnostic\n",args_file.display(),source_png.display(),source_png.display())).unwrap();
        let python = base.join("fake-python");
        fs::write(&python,format!("#!/bin/sh\nif [ \"$1\" = \"-c\" ]; then\n  case \"$2\" in\n    *platform.platform*) printf '{{\"version\":\"3.14\",\"platform\":\"fake-platform\"}}\\n' ;;\n    *get_pixmap*) cp '{}' \"$6\"; printf '{{\"width\":8,\"height\":8,\"version\":\"fake-pymupdf\"}}\\n' ;;\n    *pymupdf.open*) printf '{{\"width\":8,\"height\":8,\"version\":\"fake-pymupdf\"}}\\n' ;;\n    *) echo 3.14 ;;\n  esac\nfi\n",source_png.display())).unwrap();
        for path in [&cli, &python] {
            let mut permissions = fs::metadata(path).unwrap().permissions();
            permissions.set_mode(0o755);
            fs::set_permissions(path, permissions).unwrap();
        }
        let font_a = base.join("regular.ttf");
        let font_b = base.join("symbols.otf");
        fs::write(&font_a, b"font-a").unwrap();
        fs::write(&font_b, b"font-b").unwrap();
        let output = base.join("result");
        let fixture = Fixture {
            id: "fixture".into(),
            sdocx: sdocx.clone(),
            reference_pdf: pdf.clone(),
            sdocx_sha256: sha256(&sdocx).unwrap(),
            reference_pdf_sha256: sha256(&pdf).unwrap(),
            visible_pages: 2,
        };
        let report = run_visual(
            &[fixture],
            &VisualConfig {
                cli,
                output: output.clone(),
                format: "png".into(),
                fonts: vec![font_a.clone(), font_b.clone()],
                options: MetricsOptions::default(),
                python,
            },
            &workspace_root(),
        )
        .unwrap();
        assert_eq!(report["fixtures"][0]["pages"].as_array().unwrap().len(), 2);
        let page = &report["fixtures"][0]["pages"][0];
        assert_eq!(page["metrics"]["changed_pixel_fraction"], 0.0);
        assert!(!Path::new(page["sdk"].as_str().unwrap()).is_absolute());
        assert!(output.join("fixture/sdk.log").is_file());
        assert!(output.join("report.json").is_file());
        assert!(output.join("index.html").is_file());
        assert_eq!(report["environment"]["pymupdf"], "fake-pymupdf");
        assert_eq!(report["environment"]["platform"], "fake-platform");
        assert_eq!(report["fixtures"][0]["pages"][0]["page"], 1);
        assert_eq!(
            report["fixtures"][0]["pages"][0]["sdk"],
            "fixture/sdk_page0.png"
        );
        assert_eq!(report["fixtures"][0]["pages"][1]["page"], 2);
        assert_eq!(
            report["fixtures"][0]["pages"][1]["sdk"],
            "fixture/sdk_page1.png"
        );
        let args = fs::read_to_string(args_file).unwrap();
        let rows: Vec<_> = args.lines().collect();
        assert_eq!(
            &rows[3..],
            &[
                "--font",
                font_a.to_str().unwrap(),
                "--font",
                font_b.to_str().unwrap()
            ]
        );
        let font_hash = sha256(&font_a).unwrap();
        assert_eq!(
            report["environment"]["explicit_fonts"][0]["sha256"].as_str(),
            Some(font_hash.as_str())
        );
        fs::remove_dir_all(base).unwrap();
    }
    #[cfg(unix)]
    #[test]
    fn successful_pdf_workflow_records_pdf_text_dimensions_hash_and_download() {
        use std::os::unix::fs::PermissionsExt;
        let base = temp();
        fs::create_dir_all(&base).unwrap();
        let sdocx = base.join("source.sdocx");
        fs::write(&sdocx, b"archive").unwrap();
        let pdf = base.join("reference.pdf");
        pdf_with_pages(&pdf, 12);
        let source_png = base.join("page.png");
        // At 96 DPI an 8 point page rounds to 11 pixels on each axis.
        let mut pixels = RgbImage::blank(11, 11, [255; 3]);
        pixels.set_pixel(3, 3, [0; 3]);
        pixels.to_png(&source_png).unwrap();
        let cli = base.join("fake-cli");
        fs::write(&cli,format!("#!/bin/sh\nif [ \"$1\" = \"--version\" ]; then echo 'fake cli 1'; exit 0; fi\ncp '{}' \"$3\"\necho pdf-conversion\n",pdf.display())).unwrap();
        let python = base.join("fake-python");
        fs::write(&python,format!("#!/bin/sh\nif [ \"$1\" = \"-c\" ]; then\n case \"$2\" in\n *platform.platform*) printf '{{\"version\":\"3.14\",\"platform\":\"fake-platform\"}}\\n' ;;\n *get_pixmap*) cp '{}' \"$6\"; printf '{{\"width\":8,\"height\":8,\"version\":\"fake-pymupdf\"}}\\n' ;;\n *pymupdf.open*) printf '{{\"width\":8,\"height\":8,\"version\":\"fake-pymupdf\",\"text\":\"selectable-page%s\"}}\\n' \"$4\" ;;\n *) echo 3.14 ;;\n esac\nfi\n",source_png.display())).unwrap();
        for path in [&cli, &python] {
            let mut permissions = fs::metadata(path).unwrap().permissions();
            permissions.set_mode(0o755);
            fs::set_permissions(path, permissions).unwrap();
        }
        let output = base.join("result");
        let fixture = Fixture {
            id: "fixture".into(),
            sdocx: sdocx.clone(),
            reference_pdf: pdf.clone(),
            sdocx_sha256: sha256(&sdocx).unwrap(),
            reference_pdf_sha256: sha256(&pdf).unwrap(),
            visible_pages: 12,
        };
        let report = run_visual(
            &[fixture],
            &VisualConfig {
                cli,
                output: output.clone(),
                format: "pdf".into(),
                fonts: vec![],
                options: MetricsOptions::default(),
                python,
            },
            &workspace_root(),
        )
        .unwrap();
        let page = &report["fixtures"][0]["pages"][0];
        assert_eq!(page["sdk_pdf_size_points"], serde_json::json!([8.0, 8.0]));
        assert_eq!(page["sdk_text_characters"], 16);
        assert_eq!(
            fs::read_to_string(output.join("fixture/sdk_page0.txt")).unwrap(),
            "selectable-page0"
        );
        assert_eq!(
            fs::read_to_string(output.join("fixture/sdk_page11.txt")).unwrap(),
            "selectable-page11"
        );
        let pdf_hash = sha256(output.join("fixture/sdk.pdf")).unwrap();
        assert_eq!(
            report["fixtures"][0]["sdk_pdf_sha256"].as_str(),
            Some(pdf_hash.as_str())
        );
        assert!(output.join("fixture/sdk_page11.txt").is_file());
        assert!(
            fs::read_to_string(output.join("index.html"))
                .unwrap()
                .contains("Download SDK PDF")
        );
        fs::remove_dir_all(base).unwrap();
    }
}

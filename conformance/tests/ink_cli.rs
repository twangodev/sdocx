use std::fs::{self, File};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

struct TemporaryDirectory(PathBuf);

impl TemporaryDirectory {
    fn new() -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock is after the Unix epoch")
            .as_nanos();
        let path =
            std::env::temp_dir().join(format!("sdocx-ink-cli-{}-{nonce}", std::process::id()));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
}

impl Drop for TemporaryDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn write_rgba_png(path: &Path, width: u32, height: u32, pixels: &[[u8; 4]]) {
    let mut encoder = png::Encoder::new(File::create(path).unwrap(), width, height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().unwrap();
    let bytes: Vec<_> = pixels
        .iter()
        .flat_map(|pixel| pixel.iter().copied())
        .collect();
    writer.write_image_data(&bytes).unwrap();
}

#[test]
fn real_cli_reports_profile_region_chroma_and_white_composited_metrics() {
    let directory = TemporaryDirectory::new();
    let geometry_path = directory.0.join("geometry.json");
    let reference_path = directory.0.join("reference.png");
    let candidate_path = directory.0.join("candidate.png");
    let geometry = serde_json::json!([
        {"stroke": {"rendering": {"advanced_settings": "7;"}},
         "geometry": {"profile": "Marker4", "bounds": {"x_min": 8.0, "y_min": 8.0, "x_max": 11.0, "y_max": 11.0}}},
        {"stroke": {"rendering": {"advanced_settings": "8;"}},
         "geometry": {"profile": "Marker4", "bounds": {"x_min": 8.0, "y_min": 8.0, "x_max": 11.0, "y_max": 11.0}}},
        {"stroke": {"rendering": {"advanced_settings": "7;"}},
         "geometry": {"profile": "FountainPen", "bounds": {"x_min": 8.0, "y_min": 8.0, "x_max": 11.0, "y_max": 11.0}}},
        {"stroke": {"rendering": {"advanced_settings": "7;"}},
         "geometry": {"profile": "Marker4", "bounds": {"x_min": 100.0, "y_min": 100.0, "x_max": 103.0, "y_max": 103.0}}}
    ]);
    fs::write(&geometry_path, serde_json::to_vec(&geometry).unwrap()).unwrap();

    let mut reference = vec![[255, 255, 255, 255]; 20 * 20];
    let mut candidate = reference.clone();
    // A monochrome mark is removed by chroma selection; alpha-zero color is
    // white-composited before the chroma test.
    reference[8 * 20 + 8] = [0, 0, 0, 255];
    candidate[8 * 20 + 8] = [0, 0, 0, 255];
    reference[8 * 20 + 9] = [50, 150, 255, 0];
    candidate[8 * 20 + 9] = [50, 150, 255, 0];
    reference[10 * 20 + 10] = [50, 150, 255, 128];
    candidate[10 * 20 + 10] = [70, 140, 220, 128];
    candidate[10 * 20 + 13] = [255, 0, 0, 255];
    write_rgba_png(&reference_path, 20, 20, &reference);
    write_rgba_png(&candidate_path, 20, 20, &candidate);

    let output = Command::new(env!("CARGO_BIN_EXE_ink"))
        .arg(&geometry_path)
        .arg(&reference_path)
        .arg(&candidate_path)
        .args([
            "--profile",
            "Marker4",
            "--settings",
            "7;",
            "--region",
            "0",
            "0",
            "20",
            "20",
            "--chroma-threshold",
            "35",
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        output.stderr.is_empty(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["stroke_regions"], 1);
    assert_eq!(report["padding"], 3);
    assert_eq!(report["tolerance"], 1);
    assert_eq!(report["ink_threshold"], 32);
    assert_eq!(report["profile"], "Marker4");
    assert_eq!(report["settings"], "7;");
    assert_eq!(report["region"], serde_json::json!([0, 0, 20, 20]));
    assert_eq!(report["chroma_threshold"], 35);
    assert_eq!(report["ink_intersection_over_union"], 0.5);
    assert_eq!(report["reference_ink_pixels"], 1);
    assert_eq!(report["sdk_ink_pixels"], 2);
    assert_eq!(report["missing_ink_fraction"], 0.0);
    assert_eq!(report["extra_ink_fraction"], 0.5);
    assert_eq!(report["shared_ink_mean_rgb_error"], 11.0);
}

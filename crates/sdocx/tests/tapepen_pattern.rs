use sdocx::tapepen_pattern::{AssetError, rasterize};
use sha2::{Digest, Sha256};

fn synthetic_source() -> String {
    let mut svg = String::from(
        "<svg width=\"120\" height=\"120\" viewBox=\"0 0 120 120\" fill=\"none\" xmlns=\"http://www.w3.org/2000/svg\">\n<g clip-path=\"url(#clip0_17692_100364)\">\n<rect width=\"120\" height=\"120\" fill=\"#FFFFFF\"/>\n<g clip-path=\"url(#clip1_17692_100364)\">\n<rect width=\"120\" height=\"120\" fill=\"#FFFFFF\"/>\n",
    );
    for _ in 0..136 {
        svg.push_str("<path d=\"M10 10V110\" stroke=\"#000000\" stroke-width=\"2\" stroke-linecap=\"round\"/>\n");
    }
    svg.push_str("</g>\n</g>\n<defs>\n<clipPath id=\"clip0_17692_100364\">\n<rect width=\"120\" height=\"120\" fill=\"white\"/>\n</clipPath>\n<clipPath id=\"clip1_17692_100364\">\n<rect width=\"120\" height=\"120\" fill=\"white\"/>\n</clipPath>\n</defs>\n</svg>\n");
    svg
}

#[test]
fn rasterizes_synthetic_source_and_rejects_invalid_geometry() {
    let svg = synthetic_source();
    for size in [64, 128, 256] {
        let pixels = rasterize(&svg, size).unwrap();
        assert_eq!(pixels.len(), size * size * 4);
        assert_eq!(&pixels[0..4], &[255, 255, 255, 255]);
        let pixel = (size / 2 * size + size / 12) * 4;
        assert_eq!(&pixels[pixel..pixel + 4], &[0, 0, 0, 255]);
    }
    for broken in [
        svg.replace("M10 10V110", "MNaN 10V110"),
        svg.replace("#000000", "#XXXXXX"),
        svg.replace("M10 10V110", "M10 10V10"),
        svg.replace("stroke-width=\"2\"", "stroke-width=\"-1\""),
        svg.replace("M10 10V110", "M10 10V1e30"),
    ] {
        for size in [64, 256] {
            assert!(rasterize(&broken, size).is_err());
        }
    }
    let skipped = svg.replace("stroke-width=\"2\"", "stroke-width=\"0\"");
    assert!(
        rasterize(&skipped, 64)
            .unwrap()
            .iter()
            .all(|&byte| byte == 255)
    );
    assert!(rasterize(&skipped.replace("#000000", "#XXXXXX"), 64).is_err());
    assert!(rasterize(&skipped.replace("M10 10V110", "MNaN 10V110"), 64).is_err());
    assert!(
        rasterize(
            &svg.replace("stroke-width=\"2\"", "stroke-width=\"1e30\""),
            64
        )
        .is_err()
    );
}

#[test]
fn rejects_invalid_inputs_without_panicking() {
    assert_eq!(rasterize("", 64), Err(AssetError::UnsupportedDocument));
    assert_eq!(rasterize("", 63), Err(AssetError::UnsupportedSize));
    assert_eq!(rasterize("", 257), Err(AssetError::UnsupportedSize));
    assert_eq!(
        rasterize(&"a".repeat(100_001), 64),
        Err(AssetError::UnsupportedDocument)
    );
}

#[test]
fn retained_native_controls() {
    let Ok(path) = std::env::var("SDOCX_TAPEPEN_SOURCE") else {
        return;
    };
    let source = std::fs::read_to_string(path).unwrap();
    assert_eq!(
        format!("{:x}", Sha256::digest(source.as_bytes())),
        "9f8b208a01db838235f795226acd6e68aa42e609b06c6e171a29447f8b68322b"
    );
    assert_eq!(format!("{:x}", Sha256::digest(rasterize(&source, 256).unwrap())),
        "42abf1113cfd13c27f1e0ccb29affae62221c0888f5ffcdc2425e2169b71b13d");
    let cases = [
        (
            source.clone(),
            [
                "78a0a70b418c59c460f8791d48d7f131ad7d445f3797323eb041fd9b689ba189",
                "f290e56addf62df35c7fbd4bfb6bddaeccc047ab382c52aa9766cafb38b7d292",
            ],
        ),
        (
            source.replace("#3F5480", "#AD2244"),
            [
                "7243f4e9fb60d921b7dc94dac72a197bf5632582e23a7e0056e59baf24bdc5d5",
                "66b7258d7acb3abd69c1919f4b5739ed574f102badfe124f61340b6445647b37",
            ],
        ),
        (
            source.replace("stroke-width=\"1.7\"", "stroke-width=\"2.3\""),
            [
                "9f19ac6ab3702de697d059ab6da77b91edf233f62584dfb2e6c23a5a56ef771e",
                "76d5e8064e7ab1b93f002d54095f514dc58ed06e180ddb580861a1f131beb6bf",
            ],
        ),
    ];
    for (svg, hashes) in cases {
        for (size, hash) in [64, 128].into_iter().zip(hashes) {
            assert_eq!(
                format!("{:x}", Sha256::digest(rasterize(&svg, size).unwrap())),
                hash
            );
        }
    }
    for broken in [
        source.replace("#3F5480", "#XXXXXX"),
        source.replace("stroke-width=\"1.7\"", "stroke-width=\"1e30\""),
        source.replace("M7.495 4.5V10.5", "MNaN 4.5V10.5"),
    ] {
        assert!(rasterize(&broken, 64).is_err());
    }
}

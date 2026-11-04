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
    assert_eq!(source.matches("#3F5480").count(), 136);
    assert_eq!(source.matches("stroke-width=\"1.7\"").count(), 136);
    for (changed, expected) in [
        (
            source.replace("#3F5480", "#AD2244"),
            "198d1837f26336d46061936281addbff70f2d1e71c6d012ba748dfa564efbaf9",
        ),
        (
            source.replace("stroke-width=\"1.7\"", "stroke-width=\"2.3\""),
            "6d00dbc4b62f0e793f0a1d781c4c4876069d9ae17dfbf618df11e5a541f2d547",
        ),
    ] {
        assert_eq!(
            format!("{:x}", Sha256::digest(changed.as_bytes())),
            expected
        );
    }
    assert_eq!(
        format!("{:x}", Sha256::digest(rasterize(&source, 256).unwrap())),
        "42abf1113cfd13c27f1e0ccb29affae62221c0888f5ffcdc2425e2169b71b13d"
    );
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
        source.replace("#3F5480", "#3F548"),
        source[..source.len() - 20].to_owned(),
        format!("{source}extra\n"),
        source.replace("stroke-width=\"1.7\"", "stroke-width=\"NaN\""),
        source.replacen("M7.495 4.5V10.5", "M7.495 4.5V4.5", 1),
    ] {
        // Any panic fails the test as well as an unexpected successful result.
        assert!(rasterize(&broken, 64).is_err());
    }
    let thin = source.replace("stroke-width=\"1.7\"", "stroke-width=\"0.015\"");
    let skipped = rasterize(&thin, 64).unwrap();
    assert_eq!(skipped.len(), 64 * 64 * 4);
    assert!(
        skipped
            .as_chunks::<4>()
            .0
            .iter()
            .all(|pixel| *pixel == [200, 233, 241, 255])
    );
    let thin_128 = rasterize(&thin, 128).unwrap();
    assert_eq!(thin_128.len(), 128 * 128 * 4);
    // The 128px stroke passes the width cutoff, but upstream coverage still
    // quantizes to the source background for this very thin control.
    assert!(
        thin_128
            .as_chunks::<4>()
            .0
            .iter()
            .all(|pixel| *pixel == [200, 233, 241, 255])
    );
    // Independent upstream_texture.c output with the NanoSVG header hashes
    // pinned in texture-receipt.json; these are not native capture hashes.
    assert_eq!(
        format!("{:x}", Sha256::digest(&skipped)),
        "5ac8486144d2294bfa0345013589587410872e46938ff90f5b47eae8acee8cdb"
    );
    assert_eq!(
        format!("{:x}", Sha256::digest(&thin_128)),
        "af594704e479ac5768875b9606715c69acc2fbb5e7ccdd9337a8abb7d9bfd07d"
    );
    // This f32 width times (64 / 120) is exactly the 0.01 cutoff.
    let boundary = source.replace(
        "stroke-width=\"1.7\"",
        "stroke-width=\"0.01874999888241291\"",
    );
    assert_eq!(rasterize(&boundary, 64).unwrap(), skipped);
    for broken in [
        thin.replace("#3F5480", "#XXXXXX"),
        thin.replace("#3F5480", "#3F548"),
        thin.replacen("M7.495 4.5V10.5", "MNaN 4.5V10.5", 1),
    ] {
        assert_ne!(broken, thin);
        assert!(rasterize(&broken, 64).is_err());
    }
}

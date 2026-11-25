// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) Jörg Schröder
// ragnar-eye — decode pipeline tests.
//
// Real fixtures: we synthesize a PNG at runtime (no binary blobs
// in the repo) and exercise detection, dispatch, and the
// unsupported-format path.

use std::io::Write;

fn write_png(path: &std::path::Path) {
    // Minimal valid PNG: 1x1 red pixel, built via the `image`
    // crate so we never commit binary fixtures.
    let img = image::RgbaImage::from_pixel(1, 1, image::Rgba([255, 0, 0, 255]));
    img.save(path).unwrap();
}

fn tmpdir() -> std::path::PathBuf {
    let d = std::env::temp_dir().join(format!("ragnar-eye-test-{}", std::process::id()));
    std::fs::create_dir_all(&d).unwrap();
    d
}

#[test]
fn decodes_png() {
    let p = tmpdir().join("a.png");
    write_png(&p);
    let d = ragnar_eye_test_decode(p.to_str().unwrap()).unwrap();
    assert_eq!((d.width, d.height), (1, 1));
    assert_eq!(&d.rgba[..4], &[255, 0, 0, 255]);
}

#[test]
fn detects_extension_mismatch_by_content() {
    // A PNG named .gif must still decode — magic-first dispatch.
    let p = tmpdir().join("b.gif");
    write_png(&p);
    assert!(ragnar_eye_test_decode(p.to_str().unwrap()).is_ok());
}

#[test]
fn unsupported_format_fails_clean() {
    let p = tmpdir().join("c.bin");
    std::fs::File::create(&p)
        .unwrap()
        .write_all(b"\x00\x01\x02\x03 not an image")
        .unwrap();
    assert!(ragnar_eye_test_decode(p.to_str().unwrap()).is_err());
}

#[test]
fn truncated_file_fails_clean() {
    let p = tmpdir().join("d.png");
    std::fs::File::create(&p)
        .unwrap()
        .write_all(&[0x89, 0x50, 0x4e, 0x47])
        .unwrap();
    assert!(ragnar_eye_test_decode(p.to_str().unwrap()).is_err());
}

// The binary's decode is module-private; expose it through a
// test-visible shim in the library target.
fn ragnar_eye_test_decode(path: &str) -> Result<ragnar_eye::decode::Decoded, String> {
    ragnar_eye::decode::decode(path)
}

#[test]
fn filters_change_pixels() {
    let mut d = ragnar_eye::decode::Decoded {
        width: 2,
        height: 1,
        rgba: vec![255, 0, 0, 255, 0, 255, 0, 255],
    };
    ragnar_eye::filters::apply("greyscale", &mut d);
    // Red becomes grey, green becomes grey — R==G==B per pixel.
    assert_eq!(d.rgba[0], d.rgba[1]);
    assert_eq!(d.rgba[1], d.rgba[2]);
    let before = d.rgba.clone();
    ragnar_eye::filters::apply("invert", &mut d);
    ragnar_eye::filters::apply("invert", &mut d);
    assert_eq!(d.rgba, before); // double invert = identity
}

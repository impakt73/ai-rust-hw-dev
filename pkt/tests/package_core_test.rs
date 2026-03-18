use pkt::package_core;
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use tempfile::TempDir;
use zip::ZipArchive;

fn write_file(path: &Path, contents: &[u8]) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("create parent directory");
    }

    let mut file = File::create(path).expect("create file");
    file.write_all(contents).expect("write file");
}

fn create_test_core_source(temp_dir: &TempDir) -> PathBuf {
    let core_source = temp_dir.path().join("core-source");
    fs::create_dir_all(&core_source).expect("create core source");

    write_file(
        &core_source.join("core.json"),
        br#"{
  "core": {
    "metadata": {
      "author": "Analogue",
      "shortname": "PDP-1",
      "version": "1.0",
      "date_release": "2022-07-30"
    },
    "cores": [
      {
        "name": "default",
        "id": 0,
        "filename": "bitstream.rbf_r"
      }
    ]
  }
}"#,
    );
    write_file(&core_source.join("audio.json"), br#"{"audio":true}"#);
    write_file(&core_source.join("video.json"), br#"{"video":true}"#);
    write_file(&core_source.join("info.txt"), b"Pocket core info");
    write_file(&core_source.join("README.md"), b"Do not package me");
    write_file(
        &core_source.join("quartus_build.tcl"),
        b"Do not package me either",
    );
    write_file(
        &core_source.join("Platforms/pdp1.json"),
        br#"{"platform":"pdp1"}"#,
    );
    write_file(
        &core_source.join("Platforms/_images/pdp1.bin"),
        &[0x12, 0x34, 0x56],
    );
    write_file(
        &core_source.join("Assets/pdp1/common/bios.bin"),
        &[0xaa, 0xbb, 0xcc],
    );
    write_file(
        &core_source.join("src/fpga/ap_core.qpf"),
        b"source files should stay out of the package",
    );

    core_source
}

fn open_zip(path: &Path) -> ZipArchive<File> {
    let file = File::open(path).expect("open zip");
    ZipArchive::new(file).expect("read zip")
}

#[test]
fn test_package_core_creates_official_zip_layout() {
    let temp_dir = TempDir::new().expect("create temp dir");
    let core_source = create_test_core_source(&temp_dir);
    let input_rbf = temp_dir.path().join("input.rbf");
    let output_dir = temp_dir.path().join("out");

    write_file(&input_rbf, &[0x01, 0x80, 0x3c]);
    fs::create_dir_all(&output_dir).expect("create output directory");

    let output_zip = package_core(&input_rbf, &core_source, &output_dir).expect("package core");
    assert_eq!(
        output_zip.file_name().and_then(|name| name.to_str()),
        Some("Analogue.PDP-1_1.0_2022-07-30.zip")
    );

    let mut archive = open_zip(&output_zip);
    let mut zip_entry_names = (0..archive.len())
        .map(|index| {
            archive
                .by_index(index)
                .expect("zip entry")
                .name()
                .to_string()
        })
        .collect::<Vec<_>>();
    zip_entry_names.sort();

    assert!(zip_entry_names.contains(&"Assets/".to_string()));
    assert!(zip_entry_names.contains(&"Assets/pdp1/".to_string()));
    assert!(zip_entry_names.contains(&"Assets/pdp1/common/".to_string()));
    assert!(zip_entry_names.contains(&"Assets/pdp1/common/bios.bin".to_string()));
    assert!(zip_entry_names.contains(&"Cores/".to_string()));
    assert!(zip_entry_names.contains(&"Cores/Analogue.PDP-1/".to_string()));
    assert!(zip_entry_names.contains(&"Cores/Analogue.PDP-1/audio.json".to_string()));
    assert!(zip_entry_names.contains(&"Cores/Analogue.PDP-1/core.json".to_string()));
    assert!(zip_entry_names.contains(&"Cores/Analogue.PDP-1/info.txt".to_string()));
    assert!(zip_entry_names.contains(&"Cores/Analogue.PDP-1/video.json".to_string()));
    assert!(zip_entry_names.contains(&"Cores/Analogue.PDP-1/bitstream.rbf_r".to_string()));
    assert!(zip_entry_names.contains(&"Platforms/".to_string()));
    assert!(zip_entry_names.contains(&"Platforms/_images/".to_string()));
    assert!(zip_entry_names.contains(&"Platforms/_images/pdp1.bin".to_string()));
    assert!(zip_entry_names.contains(&"Platforms/pdp1.json".to_string()));
    assert!(!zip_entry_names
        .iter()
        .any(|name| name.ends_with("README.md")));
    assert!(!zip_entry_names
        .iter()
        .any(|name| name.ends_with("quartus_build.tcl")));
    assert!(!zip_entry_names.iter().any(|name| name.contains("src/fpga")));

    let mut bitstream = archive
        .by_name("Cores/Analogue.PDP-1/bitstream.rbf_r")
        .expect("bitstream entry");
    let mut bytes = Vec::new();
    bitstream.read_to_end(&mut bytes).expect("read bitstream");
    assert_eq!(bytes, vec![0x80, 0x01, 0x3c]);
}

#[test]
fn test_package_core_rejects_non_official_explicit_zip_name() {
    let temp_dir = TempDir::new().expect("create temp dir");
    let core_source = create_test_core_source(&temp_dir);
    let input_rbf = temp_dir.path().join("input.rbf");
    write_file(&input_rbf, &[0xff]);

    let error = package_core(
        &input_rbf,
        &core_source,
        &temp_dir.path().join("wrong-name.zip"),
    )
    .expect_err("expected invalid filename error");

    assert!(error
        .to_string()
        .contains("output zip filename must match the official Analogue naming convention"));
}

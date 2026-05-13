use std::process::Command;
use std::io::Write;

fn create_test_csv() -> tempfile::NamedTempFile {
    let mut file = tempfile::Builder::new().suffix(".csv").tempfile().unwrap();
    writeln!(file, "id,name,score").unwrap();
    writeln!(file, "1,Alice,95.5").unwrap();
    writeln!(file, "2,Bob,87.0").unwrap();
    writeln!(file, "3,Charlie,72.3").unwrap();
    file
}

#[test]
fn test_csv_to_json_roundtrip() {
    let input = create_test_csv();
    let json_out = tempfile::NamedTempFile::new().unwrap();
    let csv_out = tempfile::NamedTempFile::new().unwrap();

    let status = Command::new(env!("CARGO_BIN_EXE_ferryman"))
        .arg("convert")
        .arg("-f").arg("csv")
        .arg("-t").arg("json")
        .arg("-y")
        .arg(input.path())
        .arg(json_out.path())
        .status().unwrap();
    assert!(status.success());

    let status = Command::new(env!("CARGO_BIN_EXE_ferryman"))
        .arg("convert")
        .arg("-f").arg("json")
        .arg("-t").arg("csv")
        .arg("-y")
        .arg(json_out.path())
        .arg(csv_out.path())
        .status().unwrap();
    assert!(status.success());

    let content = std::fs::read_to_string(csv_out.path()).unwrap();
    assert!(content.lines().count() >= 3, "Expected at least 3 data rows");
}

#[test]
fn test_list_formats() {
    let output = Command::new(env!("CARGO_BIN_EXE_ferryman"))
        .arg("list-formats")
        .output().unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("csv"));
    assert!(stdout.contains("json"));
    assert!(stdout.contains("parquet"));
}

#[test]
fn test_detect_format() {
    let input = create_test_csv();
    let output = Command::new(env!("CARGO_BIN_EXE_ferryman"))
        .arg("detect")
        .arg(input.path())
        .output().unwrap();
    assert!(output.status.success());
    assert_eq!(String::from_utf8(output.stdout).unwrap().trim(), "csv");
}

#[test]
fn test_orc_write_error() {
    let input = create_test_csv();
    let result = Command::new(env!("CARGO_BIN_EXE_ferryman"))
        .arg("convert")
        .arg("-f").arg("csv")
        .arg("-t").arg("orc")
        .arg("-y")
        .arg(input.path())
        .arg("/tmp/ferryman_test_orc.orc")
        .output().unwrap();
    assert!(!result.status.success());
    let stderr = String::from_utf8(result.stderr).unwrap();
    assert!(stderr.contains("ORC write"));
}

#[test]
fn test_no_clobber() {
    let input = create_test_csv();
    let output = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(output.path(), "existing").unwrap();

    let result = Command::new(env!("CARGO_BIN_EXE_ferryman"))
        .arg("convert")
        .arg("-f").arg("csv")
        .arg("-t").arg("json")
        .arg(input.path())
        .arg(output.path())
        .arg("--no-clobber")
        .output().unwrap();
    assert!(!result.status.success());
}

#[test]
fn test_auto_detect_formats() {
    let input = create_test_csv();
    let output_dir = tempfile::tempdir().unwrap();
    let out_path = output_dir.path().join("output.json");

    let status = Command::new(env!("CARGO_BIN_EXE_ferryman"))
        .arg("convert")
        .arg("-y")
        .arg(input.path())
        .arg(&out_path)
        .status().unwrap();
    assert!(status.success());

    assert!(out_path.exists());
}

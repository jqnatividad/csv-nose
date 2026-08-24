//! Tests that execute the built binary.
//!
//! No other test in this repository runs the CLI, so the library can be
//! entirely correct while the text, JSON, or CSV serialization of its results
//! is not. These assert that each output format parses with a real parser and
//! carries the right values, rather than matching substrings.
#![cfg(feature = "cli")]

use std::io::Write;
use std::process::Command;
use tempfile::NamedTempFile;

const BIN: &str = env!("CARGO_BIN_EXE_csv-nose");

/// Windows-1252 "José,São Paulo" — not valid UTF-8, comma-delimited.
const LATIN1: &[u8] = b"name,city\nJos\xE9,S\xE3o Paulo\nRen\xE9e,Z\xFCrich\n";
/// Plain UTF-8, comma-delimited.
const UTF8: &[u8] = b"name,city\nAlice,Tokyo\nBob,Oslo\n";

fn fixture(bytes: &[u8]) -> NamedTempFile {
    let mut file = NamedTempFile::new().expect("create temp fixture");
    file.write_all(bytes).expect("write fixture");
    file.flush().expect("flush fixture");
    file
}

fn run(args: &[&str]) -> String {
    let output = Command::new(BIN).args(args).output().expect("run csv-nose");
    assert!(
        output.status.success(),
        "csv-nose exited with {}: {}",
        output.status,
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).expect("stdout is UTF-8")
}

fn column<'a>(header: &csv::StringRecord, row: &'a csv::StringRecord, name: &str) -> &'a str {
    let idx = header
        .iter()
        .position(|h| h == name)
        .unwrap_or_else(|| panic!("no `{name}` column in CSV header"));
    &row[idx]
}

#[test]
fn json_output_parses_and_escapes_the_dialect() {
    let file = fixture(LATIN1);
    let out = run(&["-f", "json", file.path().to_str().unwrap()]);

    // The delimiter and quote characters are `,` and `"`, both of which have
    // to survive JSON encoding.
    let parsed: serde_json::Value =
        serde_json::from_str(out.trim()).expect("`-f json` must emit valid JSON");

    assert_eq!(parsed["dialect"]["delimiter"], ",");
    assert_eq!(parsed["dialect"]["quote"], "\"");
    assert_eq!(parsed["dialect"]["is_utf8"], false);
    assert_eq!(parsed["encoding"]["name"], "windows-1252");
    assert_eq!(parsed["encoding"]["is_utf8"], false);
    assert_eq!(parsed["encoding"]["has_bom"], false);
}

#[test]
fn json_output_reports_valid_utf8() {
    let file = fixture(UTF8);
    let out = run(&["-f", "json", file.path().to_str().unwrap()]);

    let parsed: serde_json::Value =
        serde_json::from_str(out.trim()).expect("`-f json` must emit valid JSON");

    assert_eq!(parsed["dialect"]["is_utf8"], true);
    assert_eq!(parsed["encoding"]["name"], "UTF-8");
    assert_eq!(parsed["encoding"]["is_utf8"], true);
    assert_eq!(parsed["encoding"]["has_bom"], false);
}

#[test]
fn csv_output_parses_and_keeps_columns_aligned() {
    let file = fixture(LATIN1);
    let out = run(&["-f", "csv", file.path().to_str().unwrap()]);

    let mut reader = csv::ReaderBuilder::new()
        .has_headers(false)
        .from_reader(out.as_bytes());
    let rows: Vec<csv::StringRecord> = reader
        .records()
        .collect::<Result<_, _>>()
        .expect("`-f csv` must emit valid CSV");

    assert_eq!(rows.len(), 2, "expected a header row and one data row");
    let (header, row) = (&rows[0], &rows[1]);

    // A raw `,` delimiter or `"` quote used to be interpolated unescaped,
    // which split the data row into fewer fields than the header.
    assert_eq!(
        header.len(),
        row.len(),
        "data row has {} fields, header has {}",
        row.len(),
        header.len()
    );

    assert_eq!(column(header, row, "delimiter"), ",");
    assert_eq!(column(header, row, "quote"), "\"");
    assert_eq!(column(header, row, "is_utf8"), "false");
    assert_eq!(column(header, row, "encoding_name"), "windows-1252");
    assert_eq!(column(header, row, "encoding_is_utf8"), "false");
    assert_eq!(column(header, row, "encoding_has_bom"), "false");
}

#[test]
fn text_output_reports_encoding() {
    let latin1 = fixture(LATIN1);
    let utf8 = fixture(UTF8);

    let latin1_output = run(&[latin1.path().to_str().unwrap()]);
    assert!(
        latin1_output.contains("Encoding: windows-1252")
            && latin1_output.contains("Encoding BOM: false")
            && latin1_output.contains("UTF-8: false"),
        "text output should report a Windows-1252 file as not UTF-8"
    );
    let utf8_output = run(&[utf8.path().to_str().unwrap()]);
    assert!(
        utf8_output.contains("Encoding: UTF-8")
            && utf8_output.contains("Encoding BOM: false")
            && utf8_output.contains("UTF-8: true"),
        "text output should report a UTF-8 file as UTF-8"
    );
}

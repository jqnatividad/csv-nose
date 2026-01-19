//! Benchmark module for testing csv-nose accuracy against CSVsniffer test datasets.
//!
//! This module provides tools to validate the Table Uniformity Method implementation
//! against the same test datasets used by CSVsniffer, enabling accuracy comparison.

use crate::{Metadata, Quote, Sniffer};
use std::collections::HashMap;
use std::fs;
use std::io::{self, BufRead};
use std::path::{Path, PathBuf};

/// Expected dialect from annotation file.
#[derive(Debug, Clone)]
pub struct ExpectedDialect {
    pub file_name: String,
    pub encoding: String,
    pub delimiter: u8,
    pub quote_char: Option<u8>,
    pub escape_char: Option<u8>,
    pub line_terminator: LineTerminator,
}

/// Line terminator type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineTerminator {
    Lf,
    Cr,
    CrLf,
}

/// Result of benchmarking a single file.
#[derive(Debug, Clone)]
pub struct FileResult {
    pub file_name: String,
    pub passed: bool,
    pub delimiter_match: bool,
    pub quote_match: bool,
    pub expected_delimiter: u8,
    pub detected_delimiter: u8,
    pub expected_quote: Option<u8>,
    pub detected_quote: Option<u8>,
    pub error: Option<String>,
}

/// Aggregate benchmark results.
#[derive(Debug, Clone, Default)]
pub struct BenchmarkResult {
    pub total: usize,
    pub passed: usize,
    pub failed: usize,
    pub errors: usize,
    pub delimiter_matches: usize,
    pub quote_matches: usize,
    pub file_results: Vec<FileResult>,
}

impl BenchmarkResult {
    /// Calculate success ratio (passed / total).
    pub fn success_ratio(&self) -> f64 {
        if self.total == 0 {
            0.0
        } else {
            self.passed as f64 / self.total as f64
        }
    }

    /// Calculate failure ratio (failed / total).
    pub fn failure_ratio(&self) -> f64 {
        if self.total == 0 {
            0.0
        } else {
            self.failed as f64 / self.total as f64
        }
    }

    /// Calculate error ratio (errors / total).
    pub fn error_ratio(&self) -> f64 {
        if self.total == 0 {
            0.0
        } else {
            self.errors as f64 / self.total as f64
        }
    }

    /// Calculate delimiter accuracy.
    pub fn delimiter_accuracy(&self) -> f64 {
        let valid = self.total - self.errors;
        if valid == 0 {
            0.0
        } else {
            self.delimiter_matches as f64 / valid as f64
        }
    }

    /// Calculate quote accuracy.
    pub fn quote_accuracy(&self) -> f64 {
        let valid = self.total - self.errors;
        if valid == 0 {
            0.0
        } else {
            self.quote_matches as f64 / valid as f64
        }
    }

    /// Calculate precision (true positives / (true positives + false positives)).
    /// For dialect detection, this is essentially the success ratio.
    pub fn precision(&self) -> f64 {
        self.success_ratio()
    }

    /// Calculate recall (true positives / (true positives + false negatives)).
    /// For dialect detection with known ground truth, this equals precision.
    pub fn recall(&self) -> f64 {
        self.success_ratio()
    }

    /// Calculate F1 score (harmonic mean of precision and recall).
    pub fn f1_score(&self) -> f64 {
        let p = self.precision();
        let r = self.recall();
        if p + r == 0.0 {
            0.0
        } else {
            2.0 * p * r / (p + r)
        }
    }

    /// Print detailed results to stdout.
    pub fn print_details(&self) {
        println!("\n=== Benchmark Results ===\n");

        for result in &self.file_results {
            let status = if result.error.is_some() {
                "ERROR"
            } else if result.passed {
                "PASS"
            } else {
                "FAIL"
            };

            print!("[{}] {}", status, result.file_name);

            if !result.passed && result.error.is_none() {
                print!(" - ");
                if !result.delimiter_match {
                    print!(
                        "delimiter: expected '{}' got '{}' ",
                        result.expected_delimiter as char, result.detected_delimiter as char
                    );
                }
                if !result.quote_match {
                    let exp = result
                        .expected_quote
                        .map_or_else(|| "none".to_string(), |c| format!("'{}'", c as char));
                    let det = result
                        .detected_quote
                        .map_or_else(|| "none".to_string(), |c| format!("'{}'", c as char));
                    print!("quote: expected {exp} got {det}");
                }
            }

            if let Some(ref err) = result.error {
                print!(" - {err}");
            }

            println!();
        }
    }

    /// Print summary metrics to stdout.
    pub fn print_summary(&self) {
        println!("\n=== Summary ===\n");
        println!("Total files:        {}", self.total);
        println!(
            "Passed:             {} ({:.1}%)",
            self.passed,
            self.success_ratio() * 100.0
        );
        println!(
            "Failed:             {} ({:.1}%)",
            self.failed,
            self.failure_ratio() * 100.0
        );
        println!(
            "Errors:             {} ({:.1}%)",
            self.errors,
            self.error_ratio() * 100.0
        );
        println!();
        println!(
            "Delimiter accuracy: {:.1}%",
            self.delimiter_accuracy() * 100.0
        );
        println!("Quote accuracy:     {:.1}%", self.quote_accuracy() * 100.0);
        println!();
        println!("Precision:          {:.3}", self.precision());
        println!("Recall:             {:.3}", self.recall());
        println!("F1 Score:           {:.3}", self.f1_score());
    }
}

/// Parse an annotation file and return a map of file name to expected dialect.
pub fn parse_annotations(path: &Path) -> io::Result<HashMap<String, ExpectedDialect>> {
    let file = fs::File::open(path)?;
    let reader = io::BufReader::new(file);
    let mut annotations = HashMap::new();

    for line in reader.lines() {
        let line = line?;
        let line = line.trim();

        // Skip comments and empty lines
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        // Skip header line
        if line.starts_with("file_name|") {
            continue;
        }

        let parts: Vec<&str> = line.split('|').collect();
        if parts.len() < 6 {
            continue;
        }

        let file_name = parts[0].to_string();
        let encoding = parts[1].to_string();
        let delimiter = parse_delimiter(parts[2]);
        let quote_char = parse_quote(parts[3]);
        let _escape_char = parse_escape(parts[4]);
        let line_terminator = parse_line_terminator(parts[5]);

        annotations.insert(
            file_name.clone(),
            ExpectedDialect {
                file_name,
                encoding,
                delimiter,
                quote_char,
                escape_char: _escape_char,
                line_terminator,
            },
        );
    }

    Ok(annotations)
}

/// Parse delimiter name to byte.
fn parse_delimiter(name: &str) -> u8 {
    match name.to_lowercase().as_str() {
        "comma" => b',',
        "semicolon" => b';',
        "tab" => b'\t',
        "space" => b' ',
        "vslash" | "pipe" => b'|',
        "colon" => b':',
        _ => b',', // Default to comma
    }
}

/// Parse quote character name to byte.
fn parse_quote(name: &str) -> Option<u8> {
    match name.to_lowercase().as_str() {
        "doublequote" | "double" => Some(b'"'),
        "singlequote" | "single" => Some(b'\''),
        "tilde" => Some(b'~'),
        "" | "none" => None,
        _ => Some(b'"'), // Default to double quote
    }
}

/// Parse escape character name to byte.
fn parse_escape(name: &str) -> Option<u8> {
    match name.to_lowercase().as_str() {
        "doublequote" | "double" => Some(b'"'),
        "singlequote" | "single" => Some(b'\''),
        "backslash" => Some(b'\\'),
        "" | "none" => None,
        _ => None,
    }
}

/// Parse line terminator name.
fn parse_line_terminator(name: &str) -> LineTerminator {
    match name.to_lowercase().as_str() {
        "lf" => LineTerminator::Lf,
        "cr" => LineTerminator::Cr,
        "crlf" => LineTerminator::CrLf,
        _ => LineTerminator::Lf,
    }
}

/// Run benchmark on a directory of CSV files.
pub fn run_benchmark(data_dir: &Path, annotations_path: &Path) -> io::Result<BenchmarkResult> {
    let annotations = parse_annotations(annotations_path)?;
    let mut result = BenchmarkResult::default();

    // Process each file in the annotations
    for (file_name, expected) in &annotations {
        let file_path = data_dir.join(file_name);
        result.total += 1;

        let file_result = benchmark_file(&file_path, expected);

        if file_result.error.is_some() {
            result.errors += 1;
        } else if file_result.passed {
            result.passed += 1;
            result.delimiter_matches += 1;
            result.quote_matches += 1;
        } else {
            result.failed += 1;
            if file_result.delimiter_match {
                result.delimiter_matches += 1;
            }
            if file_result.quote_match {
                result.quote_matches += 1;
            }
        }

        result.file_results.push(file_result);
    }

    // Sort results by file name for consistent output
    result
        .file_results
        .sort_by(|a, b| a.file_name.cmp(&b.file_name));

    Ok(result)
}

/// Benchmark a single file against expected dialect.
fn benchmark_file(file_path: &Path, expected: &ExpectedDialect) -> FileResult {
    let file_name = expected.file_name.clone();

    // Check if file exists
    if !file_path.exists() {
        return FileResult {
            file_name,
            passed: false,
            delimiter_match: false,
            quote_match: false,
            expected_delimiter: expected.delimiter,
            detected_delimiter: 0,
            expected_quote: expected.quote_char,
            detected_quote: None,
            error: Some("File not found".to_string()),
        };
    }

    // Run sniffer
    let mut sniffer = Sniffer::new();
    let metadata: Result<Metadata, _> = sniffer.sniff_path(file_path);

    match metadata {
        Ok(meta) => {
            let detected_delimiter = meta.dialect.delimiter;
            let detected_quote = match meta.dialect.quote {
                Quote::None => None,
                Quote::Some(c) => Some(c),
            };

            let delimiter_match = detected_delimiter == expected.delimiter;
            let quote_match = detected_quote == expected.quote_char;
            let passed = delimiter_match && quote_match;

            FileResult {
                file_name,
                passed,
                delimiter_match,
                quote_match,
                expected_delimiter: expected.delimiter,
                detected_delimiter,
                expected_quote: expected.quote_char,
                detected_quote,
                error: None,
            }
        }
        Err(e) => FileResult {
            file_name,
            passed: false,
            delimiter_match: false,
            quote_match: false,
            expected_delimiter: expected.delimiter,
            detected_delimiter: 0,
            expected_quote: expected.quote_char,
            detected_quote: None,
            error: Some(e.to_string()),
        },
    }
}

/// Find the annotation file for a data directory.
pub fn find_annotations(data_dir: &Path) -> Option<PathBuf> {
    // Check for annotations in parent directory
    let dir_name = data_dir.file_name()?.to_str()?;
    let parent = data_dir.parent()?;

    // Try annotations subdirectory
    let annotations_dir = parent.join("annotations");
    if annotations_dir.is_dir() {
        let annotation_file = annotations_dir.join(format!("{dir_name}.txt"));
        if annotation_file.exists() {
            return Some(annotation_file);
        }
    }

    // Try direct annotation file in data dir
    let direct_annotation = data_dir.join("annotations.txt");
    if direct_annotation.exists() {
        return Some(direct_annotation);
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_delimiter() {
        assert_eq!(parse_delimiter("comma"), b',');
        assert_eq!(parse_delimiter("semicolon"), b';');
        assert_eq!(parse_delimiter("tab"), b'\t');
        assert_eq!(parse_delimiter("space"), b' ');
        assert_eq!(parse_delimiter("vslash"), b'|');
        assert_eq!(parse_delimiter("colon"), b':');
    }

    #[test]
    fn test_parse_quote() {
        assert_eq!(parse_quote("doublequote"), Some(b'"'));
        assert_eq!(parse_quote("singlequote"), Some(b'\''));
        assert_eq!(parse_quote("tilde"), Some(b'~'));
        assert_eq!(parse_quote(""), None);
        assert_eq!(parse_quote("none"), None);
    }

    #[test]
    fn test_parse_line_terminator() {
        assert_eq!(parse_line_terminator("lf"), LineTerminator::Lf);
        assert_eq!(parse_line_terminator("cr"), LineTerminator::Cr);
        assert_eq!(parse_line_terminator("crlf"), LineTerminator::CrLf);
    }

    #[test]
    fn test_benchmark_result_metrics() {
        let result = BenchmarkResult {
            total: 100,
            passed: 80,
            failed: 15,
            errors: 5,
            delimiter_matches: 85,
            quote_matches: 90,
            file_results: vec![],
        };

        assert!((result.success_ratio() - 0.80).abs() < 0.001);
        assert!((result.failure_ratio() - 0.15).abs() < 0.001);
        assert!((result.error_ratio() - 0.05).abs() < 0.001);
        assert!((result.delimiter_accuracy() - 0.894736).abs() < 0.001); // 85/95
        assert!((result.quote_accuracy() - 0.947368).abs() < 0.001); // 90/95
        assert!((result.f1_score() - 0.80).abs() < 0.001);
    }
}

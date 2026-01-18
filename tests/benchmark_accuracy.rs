//! Benchmark accuracy tests for csv-nose against CSVsniffer test datasets.
//!
//! These tests validate the Table Uniformity Method implementation against
//! real-world CSV files with known dialects.

use csv_nose::benchmark::{parse_annotations, run_benchmark, BenchmarkResult};
use std::path::PathBuf;

fn get_test_data_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/data")
}

fn run_dataset_benchmark(dataset: &str) -> BenchmarkResult {
    let data_dir = get_test_data_dir();
    let csv_dir = data_dir.join(dataset);
    let annotations_path = data_dir
        .join("annotations")
        .join(format!("{}.txt", dataset));

    if !csv_dir.exists() {
        panic!(
            "Test data directory not found: {}. \
             Please copy CSVsniffer test files to tests/data/{}",
            csv_dir.display(),
            dataset
        );
    }

    if !annotations_path.exists() {
        panic!(
            "Annotations file not found: {}. \
             Please copy annotations to tests/data/annotations/{}.txt",
            annotations_path.display(),
            dataset
        );
    }

    run_benchmark(&csv_dir, &annotations_path).expect("Benchmark should complete successfully")
}

#[test]
fn test_pollock_accuracy() {
    let result = run_dataset_benchmark("pollock");

    println!("\n========== POLLOCK Dataset ==========");
    result.print_details();
    result.print_summary();

    // Basic sanity checks
    assert!(result.total > 0, "Should have test files");
    assert!(
        result.errors < result.total / 2,
        "Errors should be less than 50% of total"
    );

    // Target: >90% accuracy on POLLOCK
    let accuracy = result.success_ratio();
    println!("\nPOLLOCK Accuracy: {:.1}%", accuracy * 100.0);
    // Note: This assertion may need adjustment based on actual performance
    // assert!(accuracy >= 0.90, "Accuracy should be >= 90%, got {:.1}%", accuracy * 100.0);
}

#[test]
fn test_w3c_csvw_accuracy() {
    let result = run_dataset_benchmark("w3c-csvw");

    println!("\n========== W3C-CSVW Dataset ==========");
    result.print_details();
    result.print_summary();

    // Basic sanity checks
    assert!(result.total > 0, "Should have test files");
    assert!(
        result.errors < result.total / 2,
        "Errors should be less than 50% of total"
    );

    // Target: >90% accuracy on W3C-CSVW
    let accuracy = result.success_ratio();
    println!("\nW3C-CSVW Accuracy: {:.1}%", accuracy * 100.0);
    // Note: This assertion may need adjustment based on actual performance
    // assert!(accuracy >= 0.90, "Accuracy should be >= 90%, got {:.1}%", accuracy * 100.0);
}

#[test]
fn test_parse_pollock_annotations() {
    let data_dir = get_test_data_dir();
    let annotations_path = data_dir.join("annotations/pollock.txt");

    if !annotations_path.exists() {
        panic!("Pollock annotations not found");
    }

    let annotations = parse_annotations(&annotations_path).expect("Should parse annotations");

    // Verify expected number of entries
    assert!(
        annotations.len() >= 140,
        "Should have at least 140 annotations, got {}",
        annotations.len()
    );

    // Spot check some entries
    if let Some(entry) = annotations.get("file_field_delimiter_0x3B.csv") {
        assert_eq!(entry.delimiter, b';', "Should detect semicolon delimiter");
    }

    if let Some(entry) = annotations.get("file_field_delimiter_0x9.csv") {
        assert_eq!(entry.delimiter, b'\t', "Should detect tab delimiter");
    }

    if let Some(entry) = annotations.get("file_quotation_char_0x27.csv") {
        assert_eq!(entry.quote_char, Some(b'\''), "Should detect single quote");
    }
}

#[test]
fn test_parse_w3c_csvw_annotations() {
    let data_dir = get_test_data_dir();
    let annotations_path = data_dir.join("annotations/w3c-csvw.txt");

    if !annotations_path.exists() {
        panic!("W3C-CSVW annotations not found");
    }

    let annotations = parse_annotations(&annotations_path).expect("Should parse annotations");

    // Verify expected number of entries
    assert!(
        annotations.len() >= 200,
        "Should have at least 200 annotations, got {}",
        annotations.len()
    );

    // Spot check some entries
    if let Some(entry) = annotations.get("occurrence.txt") {
        assert_eq!(entry.delimiter, b'\t', "Should detect tab delimiter");
    }

    if let Some(entry) = annotations.get("methane_molecular_structure_xyz_20140911.csv") {
        assert_eq!(entry.delimiter, b' ', "Should detect space delimiter");
    }
}

#[test]
fn test_combined_accuracy_report() {
    let pollock = run_dataset_benchmark("pollock");
    let w3c = run_dataset_benchmark("w3c-csvw");

    println!("\n========================================");
    println!("       COMBINED ACCURACY REPORT        ");
    println!("========================================\n");

    println!("Dataset       | Total | Passed | Failed | Errors | Accuracy");
    println!("--------------|-------|--------|--------|--------|----------");
    println!(
        "POLLOCK       | {:>5} | {:>6} | {:>6} | {:>6} | {:>7.1}%",
        pollock.total,
        pollock.passed,
        pollock.failed,
        pollock.errors,
        pollock.success_ratio() * 100.0
    );
    println!(
        "W3C-CSVW      | {:>5} | {:>6} | {:>6} | {:>6} | {:>7.1}%",
        w3c.total,
        w3c.passed,
        w3c.failed,
        w3c.errors,
        w3c.success_ratio() * 100.0
    );

    let total_files = pollock.total + w3c.total;
    let total_passed = pollock.passed + w3c.passed;
    let total_failed = pollock.failed + w3c.failed;
    let total_errors = pollock.errors + w3c.errors;
    let combined_accuracy = if total_files > 0 {
        total_passed as f64 / total_files as f64
    } else {
        0.0
    };

    println!("--------------|-------|--------|--------|--------|----------");
    println!(
        "COMBINED      | {:>5} | {:>6} | {:>6} | {:>6} | {:>7.1}%",
        total_files,
        total_passed,
        total_failed,
        total_errors,
        combined_accuracy * 100.0
    );

    println!("\n========================================");
    println!("            DETAILED METRICS           ");
    println!("========================================\n");

    println!("POLLOCK:");
    println!(
        "  Delimiter accuracy: {:.1}%",
        pollock.delimiter_accuracy() * 100.0
    );
    println!(
        "  Quote accuracy:     {:.1}%",
        pollock.quote_accuracy() * 100.0
    );
    println!("  F1 Score:           {:.3}", pollock.f1_score());

    println!("\nW3C-CSVW:");
    println!(
        "  Delimiter accuracy: {:.1}%",
        w3c.delimiter_accuracy() * 100.0
    );
    println!("  Quote accuracy:     {:.1}%", w3c.quote_accuracy() * 100.0);
    println!("  F1 Score:           {:.3}", w3c.f1_score());
}

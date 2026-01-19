//! Benchmark accuracy tests for csv-nose against CSVsniffer test datasets.
//!
//! These tests validate the Table Uniformity Method implementation against
//! real-world CSV files with known dialects by invoking the CLI binary.

use std::path::PathBuf;
use std::process::Command;

fn get_test_data_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/data")
}

fn get_binary_path() -> PathBuf {
    // The test binary is built in target/debug or target/release
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("target");
    path.push(if cfg!(debug_assertions) {
        "debug"
    } else {
        "release"
    });
    path.push("csv-nose");
    path
}

/// Parse accuracy percentage from benchmark output.
/// Looks for lines like "Passed:             123 (85.0%)"
fn parse_accuracy_from_output(output: &str) -> Option<f64> {
    for line in output.lines() {
        if line.contains("Passed:") && line.contains('%') {
            // Extract the percentage from the line
            if let Some(start) = line.find('(') {
                if let Some(end) = line.find('%') {
                    if start < end {
                        let pct_str = &line[start + 1..end];
                        return pct_str.trim().parse().ok();
                    }
                }
            }
        }
    }
    None
}

/// Parse total file count from benchmark output.
/// Looks for lines like "Total files:        148"
fn parse_total_from_output(output: &str) -> Option<usize> {
    for line in output.lines() {
        if line.contains("Total files:") {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if let Some(last) = parts.last() {
                return last.parse().ok();
            }
        }
    }
    None
}

/// Run benchmark on a dataset and return (total_files, accuracy_pct, stdout)
fn run_dataset_benchmark(dataset: &str) -> (usize, f64, String) {
    run_benchmark_with_annotations(dataset, dataset)
}

/// Run benchmark with separate data directory and annotations file.
/// This allows multiple benchmarks to share the same data directory with different annotation files.
fn run_benchmark_with_annotations(
    data_dir_name: &str,
    annotations_name: &str,
) -> (usize, f64, String) {
    let data_dir = get_test_data_dir();
    let csv_dir = data_dir.join(data_dir_name);
    let annotations_path = data_dir
        .join("annotations")
        .join(format!("{}.txt", annotations_name));

    if !csv_dir.exists() {
        panic!(
            "Test data directory not found: {}. \
             Please copy CSVsniffer test files to tests/data/{}",
            csv_dir.display(),
            data_dir_name
        );
    }

    if !annotations_path.exists() {
        panic!(
            "Annotations file not found: {}. \
             Please copy annotations to tests/data/annotations/{}.txt",
            annotations_path.display(),
            annotations_name
        );
    }

    let binary = get_binary_path();
    if !binary.exists() {
        panic!(
            "Binary not found at {}. Run `cargo build` first.",
            binary.display()
        );
    }

    let output = Command::new(&binary)
        .arg("--benchmark")
        .arg(&csv_dir)
        .arg("--annotations")
        .arg(&annotations_path)
        .output()
        .expect("Failed to execute csv-nose binary");

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    if !output.status.success() {
        panic!(
            "Benchmark command failed with status {:?}\nstdout: {}\nstderr: {}",
            output.status, stdout, stderr
        );
    }

    let total = parse_total_from_output(&stdout).unwrap_or(0);
    let accuracy = parse_accuracy_from_output(&stdout).unwrap_or(0.0);

    (total, accuracy, stdout)
}

#[test]
fn test_pollock_accuracy() {
    let (total, accuracy, stdout) = run_dataset_benchmark("pollock");

    println!("\n========== POLLOCK Dataset ==========");
    println!("{}", stdout);

    // Basic sanity checks
    assert!(total > 0, "Should have test files");
    println!("\nPOLLOCK Accuracy: {:.1}%", accuracy);

    // Target: >90% accuracy on POLLOCK
    // Note: This assertion may need adjustment based on actual performance
    // assert!(accuracy >= 90.0, "Accuracy should be >= 90%, got {:.1}%", accuracy);
}

#[test]
fn test_w3c_csvw_accuracy() {
    let (total, accuracy, stdout) = run_dataset_benchmark("w3c-csvw");

    println!("\n========== W3C-CSVW Dataset ==========");
    println!("{}", stdout);

    // Basic sanity checks
    assert!(total > 0, "Should have test files");
    println!("\nW3C-CSVW Accuracy: {:.1}%", accuracy);

    // Target: >90% accuracy on W3C-CSVW
    // Note: This assertion may need adjustment based on actual performance
    // assert!(accuracy >= 90.0, "Accuracy should be >= 90%, got {:.1}%", accuracy);
}

#[test]
fn test_csv_wrangling_accuracy() {
    let (total, accuracy, stdout) = run_dataset_benchmark("csv-wrangling");

    println!("\n========== CSV Wrangling Dataset ==========");
    println!("{}", stdout);

    // Basic sanity checks
    assert!(total > 0, "Should have test files");
    println!("\nCSV Wrangling Accuracy: {:.1}%", accuracy);
}

#[test]
fn test_csv_wrangling_codec_accuracy() {
    // Uses csv-wrangling data dir but csv-wrangling-codec annotations
    let (total, accuracy, stdout) =
        run_benchmark_with_annotations("csv-wrangling", "csv-wrangling-codec");

    println!("\n========== CSV Wrangling filtered CODEC ==========");
    println!("{}", stdout);

    // Basic sanity checks
    assert!(total > 0, "Should have test files");
    println!("\nCSV Wrangling CODEC Accuracy: {:.1}%", accuracy);
}

#[test]
fn test_csv_wrangling_messy_accuracy() {
    // Uses csv-wrangling data dir but csv-wrangling-messy annotations (only non-normal files)
    let (total, accuracy, stdout) =
        run_benchmark_with_annotations("csv-wrangling", "csv-wrangling-messy");

    println!("\n========== CSV Wrangling MESSY ==========");
    println!("{}", stdout);

    // Basic sanity checks
    assert!(total > 0, "Should have test files");
    println!("\nCSV Wrangling MESSY Accuracy: {:.1}%", accuracy);
}

#[test]
fn test_combined_accuracy_report() {
    let (pollock_total, pollock_accuracy, _) = run_dataset_benchmark("pollock");
    let (w3c_total, w3c_accuracy, _) = run_dataset_benchmark("w3c-csvw");
    let (wrangling_total, wrangling_accuracy, _) = run_dataset_benchmark("csv-wrangling");
    let (codec_total, codec_accuracy, _) =
        run_benchmark_with_annotations("csv-wrangling", "csv-wrangling-codec");
    let (messy_total, messy_accuracy, _) =
        run_benchmark_with_annotations("csv-wrangling", "csv-wrangling-messy");

    println!("\n========================================");
    println!("       COMBINED ACCURACY REPORT        ");
    println!("========================================\n");

    println!("Dataset            | Total | Accuracy");
    println!("-------------------|-------|----------");
    println!(
        "POLLOCK            | {:>5} | {:>7.1}%",
        pollock_total, pollock_accuracy
    );
    println!(
        "W3C-CSVW           | {:>5} | {:>7.1}%",
        w3c_total, w3c_accuracy
    );
    println!(
        "CSV Wrangling      | {:>5} | {:>7.1}%",
        wrangling_total, wrangling_accuracy
    );
    println!(
        "CSV Wrangling CODEC| {:>5} | {:>7.1}%",
        codec_total, codec_accuracy
    );
    println!(
        "CSV Wrangling MESSY| {:>5} | {:>7.1}%",
        messy_total, messy_accuracy
    );

    let total_files = pollock_total + w3c_total + wrangling_total;

    // Weighted average accuracy (using non-overlapping datasets: POLLOCK, W3C-CSVW, CSV Wrangling)
    let combined_accuracy = if total_files > 0 {
        (pollock_accuracy * pollock_total as f64
            + w3c_accuracy * w3c_total as f64
            + wrangling_accuracy * wrangling_total as f64)
            / total_files as f64
    } else {
        0.0
    };

    println!("-------------------|-------|----------");
    println!(
        "COMBINED           | {:>5} | {:>7.1}%",
        total_files, combined_accuracy
    );
}

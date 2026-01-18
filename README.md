# csv-nose

A Rust implementation of the [Table Uniformity Method](https://github.com/ws-garcia/CSVsniffer) for CSV dialect detection, designed as a drop-in replacement for [qsv-sniffer](https://github.com/jqnatividad/qsv-sniffer) in [qsv](https://github.com/jqnatividad/qsv).

## Background

This crate implements the algorithm from ["Detecting CSV File Dialects by Table Uniformity Measurement and Data Type Inference"](https://doi.org/10.3233/DS-240062) by W. García. The Table Uniformity Method achieves ~93% accuracy on real-world messy CSV files by:

1. Testing multiple potential dialects (delimiter × quote × line terminator combinations)
2. Scoring each dialect based on table uniformity (consistent field counts)
3. Scoring based on type detection (consistent data types within columns)
4. Selecting the dialect with the highest combined gamma score

## Installation

### As a library

```toml
[dependencies]
csv-nose = "0.1"
```

### As a CLI tool

```bash
cargo install csv-nose
```

## Library Usage

```rust
use csv_nose::{Sniffer, SampleSize};

let mut sniffer = Sniffer::new();
sniffer.sample_size(SampleSize::Records(100));

let metadata = sniffer.sniff_path("data.csv").unwrap();

println!("Delimiter: {}", metadata.dialect.delimiter as char);
println!("Has header: {}", metadata.dialect.header.has_header_row);
println!("Fields: {:?}", metadata.fields);
println!("Types: {:?}", metadata.types);
```

## CLI Usage

```bash
csv-nose data.csv                    # Sniff a single file
csv-nose *.csv                       # Sniff multiple files
csv-nose -f json data.csv            # Output as JSON
csv-nose --delimiter-only data.csv   # Output only the delimiter
csv-nose -v data.csv                 # Verbose output with field types
```

## API Compatibility

The public API mirrors qsv-sniffer for easy migration:

```rust
use csv_nose::{Sniffer, Metadata, Dialect, Header, Quote, Type, SampleSize, DatePreference};

let mut sniffer = Sniffer::new();
sniffer
    .sample_size(SampleSize::Records(50))
    .date_preference(DatePreference::MdyFormat)
    .delimiter(b',')
    .quote(Quote::Some(b'"'));
```

## Benchmarks

csv-nose is benchmarked against the same test datasets used by [CSVsniffer](https://github.com/ws-garcia/CSVsniffer), enabling direct accuracy comparison with other CSV dialect detection tools.

### Success Ratio

The table below shows the dialect detection success ratio. Accuracy is measured using only files that do not produce errors during dialect inference.

| Data set | `csv-nose` | `CSVsniffer MADSE` | `CSVsniffer` | `CleverCSV` | `csv.Sniffer` | DuckDB `sniff_csv` |
|:---------|:-----------|:-------------------|:-------------|:------------|:--------------|:-------------------|
| POLLOCK  | **95.92%** | 95.27%             | 96.55%       | 95.17%      | 96.35%        | 84.14%             |
| W3C-CSVW | **93.12%** | 94.52%             | 95.39%       | 61.11%      | 97.69%        | 99.08%             |

### Failure Ratio

The table below shows the failure ratio (errors during dialect detection) for each tool.

| Data set             | `csv-nose` | `CSVsniffer MADSE` | `CSVsniffer` | `CleverCSV` | `csv.Sniffer` | DuckDB `sniff_csv` |
|:---------------------|:-----------|:-------------------|:-------------|:------------|:--------------|:-------------------|
| POLLOCK [148 files]  | **0.68%**  | 0.00%              | 2.03%        | 2.03%       | 7.43%         | 2.03%              |
| W3C-CSVW [221 files] | **1.36%**  | 0.91%              | 1.81%        | 2.26%       | 41.18%        | 1.81%              |

### F1 Score

The F1 score is the harmonic mean of precision and recall, providing a balanced measure of dialect detection accuracy.

| Data set | `csv-nose` | `CSVsniffer MADSE` | `CSVsniffer` | `CleverCSV` | `csv.Sniffer` | DuckDB `sniff_csv` |
|:---------|:-----------|:-------------------|:-------------|:------------|:--------------|:-------------------|
| POLLOCK  | **0.953**  | 0.976              | 0.972        | 0.965       | 0.943         | 0.904              |
| W3C-CSVW | **0.919**  | 0.967              | 0.967        | 0.748       | 0.730         | 0.986              |

### Component Accuracy

csv-nose's delimiter and quote detection accuracy on each dataset:

| Data set | Delimiter Accuracy | Quote Accuracy |
|:---------|:-------------------|:---------------|
| POLLOCK  | 97.28%             | 97.96%         |
| W3C-CSVW | 99.08%             | 93.58%         |

### Benchmark Setup

The benchmark test files are not included in this repository. To run benchmarks, first clone [CSVsniffer](https://github.com/ws-garcia/CSVsniffer) and copy the test files:

```bash
# Clone CSVsniffer (if not already available)
git clone https://github.com/ws-garcia/CSVsniffer.git /path/to/CSVsniffer

# Copy test files to csv-nose
cp -r /path/to/CSVsniffer/CSV/* tests/data/pollock/
cp -r /path/to/CSVsniffer/W3C-CSVW/* tests/data/w3c-csvw/
```

### Running Benchmarks

Once the test files are in place:

```bash
# Run benchmark on POLLOCK dataset
cargo run --release -- --benchmark tests/data/pollock

# Run benchmark on W3C-CSVW dataset
cargo run --release -- --benchmark tests/data/w3c-csvw

# Run integration tests with detailed output
cargo test --test benchmark_accuracy -- --nocapture
```

## License

MIT OR Apache-2.0

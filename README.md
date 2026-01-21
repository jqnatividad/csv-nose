[![Crates.io](https://img.shields.io/crates/v/csv-nose.svg?logo=crates.io)](https://crates.io/crates/csv-nose)
[![Docs.rs](https://docs.rs/csv-nose/badge.svg)](https://docs.rs/crate/csv-nose)
![License: MIT OR Apache-2.0](https://img.shields.io/crates/l/csv-nose.svg)
[![DOI](https://zenodo.org/badge/1137017320.svg)](https://doi.org/10.5281/zenodo.18303093)

# csv-nose

A Rust port of the [Table Uniformity Method](https://github.com/ws-garcia/CSVsniffer) for CSV dialect detection.

## Background

This crate implements the algorithm from ["Detecting CSV File Dialects by Table Uniformity Measurement and Data Type Inference"](https://doi.org/10.3233/DS-240062) by [W. García](https://github.com/ws-garcia). The Table Uniformity Method achieves ~96% accuracy on real-world messy CSV files by:

1. Testing multiple potential dialects (delimiter × quote × line terminator combinations)
2. Scoring each dialect based on table uniformity (consistent field counts)
3. Scoring based on type detection (consistent data types within columns)
4. Selecting the dialect with the highest combined gamma score

## Installation

### As a library

```toml
[dependencies]
csv-nose = "0.3"
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

```bash
csv-nose -v /tmp/NYC_311_SR_2010-2020-sample-1M.csv
File: /tmp/NYC_311_SR_2010-2020-sample-1M.csv
  Delimiter: ','
  Quote: '"'
  Has header: true
  Preamble rows: 0
  Flexible: false
  UTF-8: true
  Fields: 41
  Avg record length: 1024 bytes
  Field details:
    1: Unique Key (Unsigned)
    2: Created Date (DateTime)
    3: Closed Date (DateTime)
    4: Agency (Text)
    5: Agency Name (Text)
    6: Complaint Type (Text)
    7: Descriptor (Text)
    8: Location Type (Text)
    9: Incident Zip (Unsigned)
    10: Incident Address (Text)
    11: Street Name (Text)
    12: Cross Street 1 (Text)
    13: Cross Street 2 (Text)
    14: Intersection Street 1 (Text)
    15: Intersection Street 2 (Text)
    16: Address Type (Text)
    17: City (Text)
    18: Landmark (Text)
    19: Facility Type (Text)
    20: Status (Text)
    21: Due Date (DateTime)
    22: Resolution Description (Text)
    23: Resolution Action Updated Date (DateTime)
    24: Community Board (Text)
    25: BBL (Unsigned)
    26: Borough (Text)
    27: X Coordinate (State Plane) (Unsigned)
    28: Y Coordinate (State Plane) (Unsigned)
    29: Open Data Channel Type (Text)
    30: Park Facility Name (Text)
    31: Park Borough (Text)
    32: Vehicle Type (NULL)
    33: Taxi Company Borough (NULL)
    34: Taxi Pick Up Location (Text)
    35: Bridge Highway Name (NULL)
    36: Bridge Highway Direction (NULL)
    37: Road Ramp (NULL)
    38: Bridge Highway Segment (NULL)
    39: Latitude (Float)
    40: Longitude (Float)
    41: Location (Text)
```

## API Compatibility

This library is designed as a drop-in replacement for [qsv-sniffer](https://github.com/jqnatividad/qsv-sniffer) used by [qsv](https://github.com/dathere/qsv). The public API mirrors qsv-sniffer for easy migration:

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
| POLLOCK  | **96.62%** | 95.27%             | 96.55%       | 95.17%      | 96.35%        | 84.14%             |
| W3C-CSVW | **99.10%** | 94.52%             | 95.39%       | 61.11%      | 97.69%        | 99.08%             |
| CSV Wrangling | **89.94%** | 90.50%          | 89.94%       | 87.99%      | 84.26%        | 91.62%             |
| CSV Wrangling CODEC | **89.44%** | 90.14%    | 90.14%       | 89.44%      | 84.18%        | 92.25%             |
| CSV Wrangling MESSY | **88.10%** | 89.60%    | 89.60%       | 89.60%      | 83.06%        | 91.94%             |

### Failure Ratio

The table below shows the failure ratio (errors during dialect detection) for each tool.

> **Note:** "Errors" are files that caused crashes or exceptions during processing (e.g., encoding issues, malformed data). This is distinct from "failures" where a file was successfully processed but the wrong dialect was detected. A 0% error rate means all files were processed without crashes, even if some detections were incorrect.

| Data set             | `csv-nose` | `CSVsniffer MADSE` | `CSVsniffer` | `CleverCSV` | `csv.Sniffer` | DuckDB `sniff_csv` |
|:---------------------|:-----------|:-------------------|:-------------|:------------|:--------------|:-------------------|
| POLLOCK [148 files]  | **0.00%**  | 0.00%              | 2.03%        | 2.03%       | 7.43%         | 2.03%              |
| W3C-CSVW [221 files] | **0.00%**  | 0.91%              | 1.81%        | 2.26%       | 41.18%        | 1.81%              |
| CSV Wrangling [179 files] | **0.00%** | 0.00%         | 0.56%        | 0.56%       | 39.66%        | 0.00%              |
| CSV Wrangling CODEC [142 files] | **0.00%** | 0.00%   | 0.00%        | 0.00%       | 38.03%        | 0.00%              |
| CSV Wrangling MESSY [126 files] | **0.00%** | 0.79%   | 0.79%        | 0.79%       | 42.06%        | 0.79%              |

### F1 Score

The F1 score is the harmonic mean of precision and recall, providing a balanced measure of dialect detection accuracy.

| Data set | `csv-nose` | `CSVsniffer MADSE` | `CSVsniffer` | `CleverCSV` | `csv.Sniffer` | DuckDB `sniff_csv` |
|:---------|:-----------|:-------------------|:-------------|:------------|:--------------|:-------------------|
| POLLOCK  | **0.966**  | 0.976              | 0.972        | 0.965       | 0.943         | 0.904              |
| W3C-CSVW | **0.991**  | 0.967              | 0.967        | 0.748       | 0.730         | 0.986              |
| CSV Wrangling | **0.899** | 0.950             | 0.945        | 0.935       | 0.724         | 0.956              |
| CSV Wrangling CODEC | **0.894** | 0.948       | 0.948        | 0.944       | 0.728         | 0.959              |
| CSV Wrangling MESSY | **0.881** | 0.943       | 0.943        | 0.943       | 0.705         | 0.956              |

### Component Accuracy

csv-nose's delimiter and quote detection accuracy on each dataset:

| Data set | Delimiter Accuracy | Quote Accuracy |
|:---------|:-------------------|:---------------|
| POLLOCK  | 96.62%             | 100.00%        |
| W3C-CSVW | 99.10%             | 100.00%        |
| CSV Wrangling | 93.30%         | 96.65%         |
| CSV Wrangling CODEC | 92.96%   | 96.48%         |
| CSV Wrangling MESSY | 92.06%   | 96.03%         |

> NOTE: See [PERFORMANCE.md](PERFORMANCE.md) for details on accuracy breakdowns and known limitations.

### Benchmark Setup

The benchmark test files are not included in this repository. To run benchmarks, first clone [CSVsniffer](https://github.com/ws-garcia/CSVsniffer) and copy the test files:

```bash
# Clone CSVsniffer (if not already available)
git clone https://github.com/ws-garcia/CSVsniffer.git /path/to/CSVsniffer

# Copy test files to csv-nose
cp -r /path/to/CSVsniffer/CSV/* tests/data/pollock/
cp -r /path/to/CSVsniffer/W3C-CSVW/* tests/data/w3c-csvw/
cp -r "/path/to/CSVsniffer/CSV_Wrangling/data/github/Curated files/"* tests/data/csv-wrangling/
```

### Running Benchmarks

Once the test files are in place:

```bash
# Run benchmark on POLLOCK dataset
cargo run --release -- --benchmark tests/data/pollock

# Run benchmark on W3C-CSVW dataset
cargo run --release -- --benchmark tests/data/w3c-csvw

# Run benchmark on CSV Wrangling dataset (all 179 files)
cargo run --release -- --benchmark tests/data/csv-wrangling

# Run benchmark on CSV Wrangling filtered CODEC (142 files)
cargo run --release -- --benchmark tests/data/csv-wrangling --annotations tests/data/annotations/csv-wrangling-codec.txt

# Run benchmark on CSV Wrangling MESSY (126 non-normal files)
cargo run --release -- --benchmark tests/data/csv-wrangling --annotations tests/data/annotations/csv-wrangling-messy.txt

# Run integration tests with detailed output
cargo test --test benchmark_accuracy -- --nocapture
```

## License

MIT OR Apache-2.0

## Naming

The name "csv-nose" is a play on words, combining "CSV" (Comma-Separated Values) with "nose," suggesting the tool's ability to "sniff out" the correct CSV dialect. "Nose" also sounds like "knows," implying expertise in CSV dialect detection.

## AI Contributions

Claude Code using Opus 4.5 was used to assist in code generation and documentation. All AI-generated content has been reviewed and edited by human contributors to ensure accuracy and quality.

[![Crates.io](https://img.shields.io/crates/v/csv-nose.svg?logo=crates.io)](https://crates.io/crates/csv-nose)
[![Docs.rs](https://docs.rs/csv-nose/badge.svg)](https://docs.rs/crate/csv-nose)
![License: MIT](https://img.shields.io/crates/l/csv-nose.svg)
[![DOI](https://zenodo.org/badge/1137017320.svg)](https://doi.org/10.5281/zenodo.18303093)

# csv-nose

A Rust port of the [Table Uniformity Method](https://github.com/ws-garcia/CSVsniffer) for CSV dialect detection.

## Background

This crate [implements](docs/IMPLEMENTATION.md) the algorithm from ["Detecting CSV File Dialects by Table Uniformity Measurement and Data Type Inference"](https://doi.org/10.3233/DS-240062)[^1] by [W. García](https://github.com/ws-garcia).

Benchmarked against the standard CSVsniffer test suites, **csv-nose is the most accurate and robust of the dialect sniffers compared**; it **leads outright on every dataset** on both success ratio and F1; is the **only tool to have 100% quote detection accuracy on the W3C-CSVW and POLLOCK test suites**; and the **only tool that never errors on a single file**.[^bench]

This implementation of the Table Uniformity Method achieves 99.55%[^2] accuracy on the [W3C-CSVW test suite](https://github.com/w3c/csvw) by:

1. Testing multiple potential dialects (delimiter × quote × line terminator combinations)
2. Scoring each dialect based on table uniformity (consistent field counts)
3. Scoring based on type detection (consistent data types within columns)
4. Selecting the dialect with the highest combined gamma score

[^1]: García W. Detecting CSV file dialects by table uniformity measurement and data type inference. Data Science. 2024;7(2):55-72. [doi:10.3233/DS-240062](https://doi.org/10.3233/DS-240062)

[^bench]: Across the five CSVsniffer benchmark suites — POLLOCK, W3C-CSVW, and the CSV Wrangling set with its CODEC and MESSY subsets — csv-nose ranks first among the six tools compared on both success ratio and F1 on every suite, and is the only tool with a 0% error rate on every suite. As of v1.2.0 this includes the filtered CODEC and MESSY subsets, where DuckDB's `sniff_csv` previously tied csv-nose on CODEC and narrowly led on MESSY (while placing last on POLLOCK); both now trail. All F1 figures are derived with a single consistent methodology from each tool's success and error ratios. See the [Accuracy Benchmarks](#accuracy-benchmarks) tables for full per-tool figures.

## Installation

### As a library

```bash
cargo add csv-nose
```

### As a CLI tool

```bash
cargo install csv-nose
```

### With HTTP support (for remote URLs)

```bash
cargo install csv-nose --features http
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
csv-nose https://example.com/data.csv  # Sniff remote CSV (requires http feature)
csv-nose local.csv https://example.com/remote.csv  # Mix local and remote
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
  Avg record length: 547 bytes
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

## Remote URL Support

When built with the `http` feature, csv-nose can sniff remote CSV files directly from URLs:

```bash
# Build with HTTP support
cargo build --release --features http

# Sniff remote CSV
csv-nose https://raw.githubusercontent.com/datasets/gdp/main/data/gdp.csv

# Limit bytes fetched (useful for large remote files)
csv-nose -b 8192 https://example.com/large.csv
```

The HTTP feature uses Range requests when supported by the server to minimize data transfer. If the server doesn't support Range requests, it falls back to downloading and truncating at the sample size limit.

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

## Accuracy Benchmarks

csv-nose is benchmarked against the [same test datasets](docs/BENCHMARK_DATASETS_INFO.md) used by [CSVsniffer](https://github.com/ws-garcia/CSVsniffer), enabling direct accuracy comparison with other CSV dialect detection tools.

### Success Ratio

The table below shows the dialect detection success ratio. Accuracy is measured using only files that do not produce errors during dialect inference.

| Data set | `csv-nose` | `CSVsniffer MADSE` | `CSVsniffer` | `CleverCSV` | `csv.Sniffer` | DuckDB `sniff_csv` |
|:---------|:-----------|:-------------------|:-------------|:------------|:--------------|:-------------------|
| POLLOCK  | **98.65%** | 95.27%             | 96.55%       | 95.17%      | 96.35%        | 84.14%             |
| W3C-CSVW[^2] | **99.55%** | 94.52%             | 95.39%       | 61.11%      | 97.69%        | 99.08%             |
| CSV Wrangling | **94.97%** | 90.50%          | 89.94%       | 87.99%      | 84.26%        | 91.62%             |
| CSV Wrangling CODEC | **94.37%** | 90.14%    | 90.14%       | 89.44%      | 84.18%        | 92.25%             |
| CSV Wrangling MESSY | **93.65%** | 89.60%    | 89.60%       | 89.60%      | 83.06%        | 91.94%             |

[^2]: csv-nose is optimized for the [W3C CSV on the Web Test Suite](https://w3c.github.io/csvw/tests/) - reaching 99.55% accuracy.

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

The F1 score is the harmonic mean of precision and recall. To compare every tool on equal footing, F1 is derived consistently from the success and error ratios above[^3]:

- **precision** = correct detections ÷ files processed without error
- **recall** = correct detections ÷ *all* files (files that error count as missed detections)

This penalizes a tool for both wrong detections and processing errors, so F1 equals the success ratio for any tool with a 0% error rate (e.g. `csv-nose` on every dataset).

| Data set | `csv-nose` | `CSVsniffer MADSE` | `CSVsniffer` | `CleverCSV` | `csv.Sniffer` | DuckDB `sniff_csv` |
|:---------|:-----------|:-------------------|:-------------|:------------|:--------------|:-------------------|
| POLLOCK  | **0.986**  | 0.953              | 0.956        | 0.942       | 0.926         | 0.833              |
| W3C-CSVW | **0.995**  | 0.941              | 0.945        | 0.604       | 0.724         | 0.982              |
| CSV Wrangling | **0.950** | 0.905             | 0.897        | 0.877       | 0.634         | 0.916              |
| CSV Wrangling CODEC | **0.944** | 0.901       | 0.901        | 0.894       | 0.644         | 0.923              |
| CSV Wrangling MESSY | **0.937** | 0.892       | 0.892        | 0.892       | 0.609         | 0.916              |

[^3]: F1 = 2·P·R/(P+R) with the precision/recall definitions above, i.e. F1 = 2·s·(1−e)/(2−e) where *s* is the success ratio and *e* the error ratio. Competitor F1 values are derived from their reported success/error ratios (sourced from the [CSVsniffer](https://github.com/ws-garcia/CSVsniffer) benchmark) using this same formula, rather than reproduced from a separate measurement, so all columns are directly comparable.

### Component Accuracy

csv-nose's delimiter and quote detection accuracy on each dataset:

| Data set | Delimiter Accuracy | Quote Accuracy |
|:---------|:-------------------|:---------------|
| POLLOCK  | 98.65%             | 100.00%        |
| W3C-CSVW | 99.55%             | 100.00%        |
| CSV Wrangling | 95.53%         | 99.44%         |
| CSV Wrangling CODEC | 95.07%   | 99.30%         |
| CSV Wrangling MESSY | 94.44%   | 99.21%         |

> NOTE: See [ACCURACY.md](docs/ACCURACY.md) for details on accuracy breakdowns and known limitations.

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

MIT license ([LICENSE-MIT](LICENSE-MIT) or <https://opensource.org/licenses/MIT>)


## Naming

The name "csv-nose" is a play on words, combining "CSV" (Comma-Separated Values) with "nose," suggesting the tool's ability to "sniff out" the correct CSV dialect. "Nose" also sounds like "knows," implying expertise in CSV dialect detection.

## AI Contributions

Claude Code using Opus 4.5 was used to assist in code generation and documentation. All AI-generated content has been reviewed and edited by human contributors to ensure accuracy and quality.

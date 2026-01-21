# Benchmark Datasets Information

This document describes the benchmark datasets used to evaluate csv-nose's dialect detection accuracy. These datasets are sourced from [CSVsniffer](https://github.com/ws-garcia/CSVsniffer) and represent a range of CSV file characteristics.

## Dataset Overview

| Dataset | Files | Source | Purpose |
|---------|-------|--------|---------|
| **POLLOCK** | 148 | Synthetic + curated | Test edge cases and controlled variations |
| **W3C-CSVW** | 221 | W3C CSV on the Web | Standardized test suite for CSV parsing |
| **CSV Wrangling** | 179 | Real-world GitHub files | Messy, real-world CSV files |
| **CSV Wrangling CODEC** | 142 | Filtered subset | Files that other tools can parse |
| **CSV Wrangling MESSY** | 126 | Filtered subset | Non-normal/problematic files |

## Delimiter Distribution

| Delimiter | POLLOCK | W3C-CSVW | CSV Wrangling |
|-----------|---------|----------|---------------|
| Comma | 104 (70%) | 219 (99%) | 127 (71%) |
| Semicolon | 39 (26%) | 0 | 36 (20%) |
| Tab | 3 (2%) | 1 (<1%) | 7 (4%) |
| Pipe (`\|`) | 1 (<1%) | 0 | 4 (2%) |
| Section sign (§) | 0 | 0 | 3 (2%) |
| Space | 1 (<1%) | 1 (<1%) | 2 (1%) |

## Quote Character Distribution

| Quote | POLLOCK | W3C-CSVW | CSV Wrangling |
|-------|---------|----------|---------------|
| Double (`"`) | 145 (98%) | 221 (100%) | 174 (97%) |
| Single (`'`) | 3 (2%) | 0 | 5 (3%) |

## Encoding Diversity

| Encoding | POLLOCK | W3C-CSVW | CSV Wrangling |
|----------|---------|----------|---------------|
| UTF-8 | 119 (80%) | 219 (99%) | 159 (89%) |
| ASCII | 29 (20%) | 0 | 1 (<1%) |
| ANSI | 0 | 2 (1%) | 12 (7%) |
| Windows-1251 | 0 | 0 | 3 (2%) |
| Other (UTF-16, Shift-JIS, GB2312) | 0 | 0 | 4 (2%) |

## Dataset Characteristics

### POLLOCK

Synthetic test files designed to test specific edge cases:

- **Multi-table files**: Files containing multiple embedded tables
- **Preamble handling**: Files with header comments or metadata rows
- **Unusual delimiters**: Space, pipe, and semicolon-delimited files
- **Escape character variations**: Different escape character configurations
- **Row anomalies**: Files with inconsistent field counts in specific rows
- **Quote variations**: Single-quote and double-quote files

Mix of simple well-formed CSVs and files with intentional structural challenges.

### W3C-CSVW

Files from the [W3C CSV on the Web](https://github.com/w3c/csvw) test suite:

- **Highly standardized**: 99% comma-delimited, 100% double-quoted, 99% UTF-8
- **Uniform dialect**: Designed for the W3C CSV on the Web specification
- **Structural variations**: Tests many small structural edge cases across 221 files
- **Line ending diversity**: Mix of LF and CRLF line endings
- **Well-documented**: Each file has a clear expected behavior

Best dataset for testing standard CSV compliance.

### CSV Wrangling

Real-world CSV files scraped from GitHub repositories:

- **Most diverse**: Wide range of encodings, delimiters, and structures
- **Real-world messiness**: Files created by various tools and humans
- **Encoding variety**: Includes UTF-16, Shift-JIS, GB2312, Windows-1251
- **Rare delimiters**: Section sign (§), pipe, and other unusual separators
- **Annotation issues**: Some expected dialects may not match actual file content

The CODEC and MESSY subsets filter this dataset:

- **CODEC (142 files)**: Files that standard CSV tools can successfully parse
- **MESSY (126 files)**: Files with non-normal structures or problematic content

## Implications for Detection

### Why W3C-CSVW has highest accuracy (99.55%)

- Uniform dialect (comma + double-quote) matches csv-nose's default preferences
- Well-formed files with clear quoting patterns
- Boundary detection works well with consistent structure

### Why CSV Wrangling has lower accuracy (~87%)

- Delimiter diversity: 29% use non-comma delimiters
- Rare delimiters (§, space) have detection penalties
- Some annotation errors in expected dialects
- Real-world files often have ambiguous structures

### Why POLLOCK is in between (96.62%)

- Intentionally challenging edge cases
- Tests specific failure modes
- Semicolon-heavy (26%) but well-structured

## Running Benchmarks

See [README.md](README.md#benchmark-setup) for instructions on setting up and running benchmarks.

```bash
# Run all benchmarks
cargo test --test benchmark_accuracy -- --nocapture

# Run individual dataset
cargo run --release -- --benchmark tests/data/pollock
cargo run --release -- --benchmark tests/data/w3c-csvw
cargo run --release -- --benchmark tests/data/csv-wrangling
```

# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Build Commands

```bash
cargo build              # Build debug
cargo build --release    # Build optimized release
cargo test               # Run all tests (unit + integration + doc tests)
cargo test test_name     # Run single test by name
cargo run -- file.csv    # Run CLI on a file
cargo clippy             # Lint
cargo fmt                # Format code
```

## Benchmark Commands

```bash
# Run benchmark on POLLOCK dataset (148 files)
cargo run --release -- --benchmark tests/data/pollock

# Run benchmark on W3C-CSVW dataset (221 files)
cargo run --release -- --benchmark tests/data/w3c-csvw

# Run benchmark with custom annotations file
cargo run --release -- --benchmark tests/data/pollock --annotations tests/data/annotations/pollock.txt

# Run benchmark integration tests with output
cargo test --test benchmark_accuracy -- --nocapture
```

Note: Benchmark test files must be copied from [CSVsniffer](https://github.com/ws-garcia/CSVsniffer). See README.md "Benchmark Setup" section.

## Architecture

csv-nose is a CSV dialect sniffer implementing the **Table Uniformity Method** from "Detecting CSV File Dialects by Table Uniformity Measurement and Data Type Inference" (García, 2024). It provides both a library (`csv_nose`) and CLI binary (`csv-nose`).

### Core Algorithm Flow

1. **`Sniffer`** (`src/sniffer.rs`) - Entry point. Reads sample data, generates potential dialects, scores them, returns `Metadata`

2. **TUM Pipeline** (`src/tum/`):
   - `potential_dialects.rs` - Generates dialect candidates (delimiter × quote × line terminator combinations)
   - `table.rs` - Parses data into a `Table` struct with rows and field counts
   - `uniformity.rs` - Computes tau_0 (consistency) and tau_1 (dispersion) scores
   - `type_detection.rs` - Detects cell types and computes type consistency scores
   - `score.rs` - Combines uniformity and type scores into gamma score, selects best dialect with delimiter/quote preference tiebreakers
   - `regexes.rs` - Lazy-compiled regex patterns for type detection

3. **Output Types** (`src/metadata.rs`):
   - `Metadata` - Full sniff result (dialect, fields, types)
   - `Dialect` - Delimiter, quote char, header info, flexibility
   - `Quote` - Quote character enum (`None` or `Some(u8)`)

4. **Benchmark Module** (`src/benchmark.rs`):
   - Parses CSVsniffer annotation files (pipe-delimited format)
   - Runs dialect detection against test datasets
   - Calculates accuracy metrics (precision, recall, F1 score)
   - Used by CLI `--benchmark` flag and integration tests

### Key Design Decisions

- **qsv-sniffer API compatibility**: The public API mirrors qsv-sniffer for drop-in replacement
- **Gamma scoring**: Dialects ranked by combined score = uniformity × type consistency × bonuses/penalties
- **Delimiter preference**: When scores are close (within 10%), prefer common delimiters (`,` > `;` > `\t` > `|`) over rare ones (`#`, `&`, space)
- **Quote preference**: When scores are close, prefer `"` over `'` over `None`
- **Header detection**: Heuristic-based (type differences between first row and data, uniqueness, length)
- **Sampling**: Configurable via `SampleSize::Records(n)`, `SampleSize::Bytes(n)`, or `SampleSize::All`

### Test Data

- `tests/data/annotations/` - Dialect annotation files (checked in)
- `tests/data/pollock/` - POLLOCK test CSVs (gitignored, copy from CSVsniffer)
- `tests/data/w3c-csvw/` - W3C-CSVW test CSVs (gitignored, copy from CSVsniffer)

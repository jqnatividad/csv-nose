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

## Architecture

csv-nose is a CSV dialect sniffer implementing the **Table Uniformity Method** from "Wrangling Messy CSV Files by Detecting Row and Type Patterns" (van den Burg et al., 2019). It provides both a library (`csv_nose`) and CLI binary (`csv-nose`).

### Core Algorithm Flow

1. **`Sniffer`** (`src/sniffer.rs`) - Entry point. Reads sample data, generates potential dialects, scores them, returns `Metadata`

2. **TUM Pipeline** (`src/tum/`):
   - `potential_dialects.rs` - Generates dialect candidates (delimiter × quote × line terminator combinations)
   - `table.rs` - Parses data into a `Table` struct with rows and field counts
   - `uniformity.rs` - Computes tau_0 (consistency) and tau_1 (dispersion) scores
   - `type_detection.rs` - Detects cell types and computes type consistency scores
   - `score.rs` - Combines uniformity and type scores into gamma score, selects best dialect
   - `regexes.rs` - Lazy-compiled regex patterns for type detection

3. **Output Types** (`src/metadata.rs`):
   - `Metadata` - Full sniff result (dialect, fields, types)
   - `Dialect` - Delimiter, quote char, header info, flexibility
   - `Quote` - Quote character enum (`None` or `Some(u8)`)

### Key Design Decisions

- **qsv-sniffer API compatibility**: The public API mirrors qsv-sniffer for drop-in replacement
- **Gamma scoring**: Dialects ranked by combined score = uniformity × type consistency × bonuses/penalties
- **Header detection**: Heuristic-based (type differences between first row and data, uniqueness, length)
- **Sampling**: Configurable via `SampleSize::Records(n)`, `SampleSize::Bytes(n)`, or `SampleSize::All`

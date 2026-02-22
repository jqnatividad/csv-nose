# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [1.0.0] - 2026-02-21

### Performance

- Parallel dialect scoring via `rayon::par_iter` with thread-local `TypeScoreBuffers` for multi-core speedup
- `#[inline]` on `detect_cell_type` hot path
- Float regex gating with cheap `.contains('.')` / `.contains('e')` checks before regex evaluation
- `TypeScoreBuffers` struct eliminates per-call heap allocations in type scoring
- `normalize_line_endings` moved before `QuoteBoundaryCounts::new` to avoid redundant work
- `cached_modal_field_count_freq` field on `Table` avoids repeated filter+count in `calculate_tau_1`
- `Cow<Table>` in `build_metadata` avoids clone in the no-preamble case

### Changed

- Bump `clap` from 4.5.56 to 4.5.60
- Bump `ureq` from 3.1.4 to 3.2.0
- Bump `regex` from 1.12.2 to 1.12.3
- Bump `tempfile` from 3.24.0 to 3.25.0
- Rename `docs/PERFORMANCE.md` to `docs/ACCURACY.md` and update accuracy figures to v1.0.0

### Fixed

- CSV Wrangling accuracy improved from 87.15% to 92.74%:
  - Fix `nsign` benchmark annotation mapping (`"nsign"` → `b'#'`, not `b'§'`)
  - Raise pipe delimiter priority to prevent space-delimiter false positives
  - Double-quote 2.2× density check now requires real quote density (not just boundary count)
  - Single-quote opening boundary requirement prevents apostrophe-in-content false positives
- Cap `Records`-mode buffer allocations at 100 MB; use probe read to avoid false-positive truncation warnings
- Restore first-maximum tie-breaking semantics and fix related correctness issues
- Address HIGH, MEDIUM, and LOW security audit findings
- Dampen false quote boost from JSON content in unquoted fields
- Fix `isco.csv` and `uniq_nl_data.csv` detection (closing-only boundary boost, space+empty-first-field penalty)
- Fix misleading comment on float regex gate in `type_detection`
- Correct `lib.rs` paper citation to García (2024) Table Uniformity Method
- Fix tiebreaking threshold comment: 10% → 5% (`score_ratio > 0.95`)
- Fix accuracy figures in `docs/BENCHMARK_DATASETS_INFO.md` (CSV Wrangling ~87%→~93%, POLLOCK 96.62%→97.30%)
- Fix `docs/IMPLEMENTATION.md` NULL specificity weight (empty string=0.0, null-like strings=0.5)

### Added

- `docs/IMPLEMENTATION.md` — comprehensive algorithm reference covering all scoring details, thresholds, and design decisions
- `docs/ACCURACY.md` — accuracy summary and known limitations (replaces `PERFORMANCE.md`, updated to v1.0.0)
- Claude Code automations: clippy pre-tool hook, `api-compat-checker` subagent, `cargo-audit` and `benchmark` skills, benchmark regression checker subagent

**Full Changelog**: https://github.com/jqnatividad/csv-nose/compare/0.8.0...v1.0.0

## [0.8.0] - 2026-01-30

### Performance

- Set CSV reader buffer capacity to 32KB (from default 8KB) for improved parsing performance with larger sample data
- Add `#[inline]` to `Quote::char` method
- Optimize `is_boolean` check in type detection pipeline
- Make `is_numeric` and `is_temporal` const functions for compile-time optimization

### Changed

- Bump `clap` from 4.5.55 to 4.5.56

### Fixed

- Remove incorrect citation in documentation

**Full Changelog**: https://github.com/jqnatividad/csv-nose/compare/v0.7.0...v0.8.0

## [0.7.0] - 2026-01-28

### Changed

- Bump `clap` from 4.5.54 to 4.5.55
- Bump `actions/checkout` from 4 to 6 in CI workflow
- Updated Dependabot config for Cargo and GitHub Actions

### Performance

- Optimize type detection by using array for type counts instead of HashMap
- Make several methods `const` for improved compile-time optimization
- Optimize hot paths for ~18-21% speedup in dialect detection

No change in detection accuracy - these are pure performance optimizations.

**Full Changelog**: https://github.com/jqnatividad/csv-nose/compare/v0.6.0...v0.7.0

## [0.6.0] - 2026-01-26

### Changed

- Replaced `std::collections::HashMap` with `foldhash::HashMap` for faster hashing
- Updated package description to credit @ws-garcia's Table Uniformity Method

### Performance

- Pre-allocate HashMap capacities to avoid reallocation during growth
- Add `#[inline]` to Table accessor methods for better optimization
- Pass Table by reference to avoid unnecessary cloning
- Use `fmt::Write` instead of `format!()` to avoid temporary string allocations

No change in detection accuracy - these are pure performance optimizations.

**Full Changelog**: https://github.com/jqnatividad/csv-nose/compare/v0.5.0...v0.6.0

## [0.5.0] - 2026-01-22

### Added

- HTTP support for sniffing remote CSV files directly from URLs without downloading the entire file
- New `http` feature flag with `ureq` dependency for HTTP Range request support
- CLI can now accept URLs alongside local file paths (e.g., `csv-nose local.csv https://example.com/remote.csv`)
- Efficient Range request handling: uses partial downloads when server supports it, falls back to
  full download with truncation otherwise

### Fixed

- Properly escape paths and URLs in JSON and CSV output formats

**Full Changelog**: https://github.com/jqnatividad/csv-nose/compare/v0.4.0...v0.5.0

## [0.4.0] - 2026-01-21

### Added

- `BENCHMARK_DATASETS_INFO.md` documenting benchmark dataset characteristics and implications for detection
- Regression test with NYC 311 sample data (`tests/data/fixtures/nyc_311_sample_200.csv`) to prevent
  future `avg_record_len` calculation bugs

### Changed

- Quote detection now uses boundary analysis - quotes must appear at field boundaries (after delimiter/newline
  or before delimiter/newline) to receive a boost, improving accuracy for standardized files
- Tightened tiebreaker threshold from 90% to 95% for delimiter/quote priority decisions
- Reduced small sample penalties (< 3 rows: 0.80 instead of 0.70; 3-4 rows: 0.90 instead of 0.85)
- Increased section sign (`§`) delimiter factor from 0.70 to 0.78 (rare but legitimate delimiter)
- Increased double-quote density boost from 1.03 to 1.06/1.08/1.15/2.2 based on boundary evidence

### Fixed

- Quote boundary detection now uses the dialect's actual delimiter instead of hardcoded values
- `avg_record_len` now correctly calculates using only the bytes consumed
  by parsed rows, not the entire sample buffer. `SampleSize::Records(n)` was always producing ~1024
  bytes because the buffer size estimate (`n * 1024`) was divided by the parsed row count (`n`)

### Performance

Significant improvement for standardized CSV files (W3C-CSVW +6.34%), with tradeoff on real-world
messy files (CSV Wrangling datasets -3.9% to -4.8%).

| Dataset | v0.3.x | v0.4.0 | Change |
|:--------|:-------|:-------|:-------|
| POLLOCK | 96.62% | 96.62% | — |
| W3C-CSVW | 93.21% | 99.55% | +6.34% |
| CSV Wrangling | 91.06% | 87.15% | -3.91% |
| CSV Wrangling CODEC | 90.85% | 86.62% | -4.23% |
| CSV Wrangling MESSY | 89.68% | 84.92% | -4.76% |

**Full Changelog**: https://github.com/jqnatividad/csv-nose/compare/v0.3.1...v0.4.0

## [0.3.1] - 2026-01-20

### Fixed

- `avg_record_len` was always ~1024 bytes regardless of actual record size. Now correctly
  calculates from parsed table data (sum of field lengths plus delimiter and line terminator overhead).

### Performance

No change in detection accuracy.

**Full Changelog**: https://github.com/jqnatividad/csv-nose/compare/v0.3.0...v0.3.1

## [0.3.0] - 2026-01-19

### Added

- `ACCURACY.md` documenting detection accuracy, known limitations, and workarounds

### Changed

- Applied select Clippy suggestions for cleaner code

### Fixed

- Non-deterministic benchmark results caused by HashMap iteration order in modal
  field count calculation. Tie-breaking is now deterministic (prefers higher field count).

### Performance

- Cache pattern categories via LazyLock (eliminates ~25,500 Vec allocs/sniff)
- Pre-compute quote counts once for all dialect evaluations (eliminates 26 scans)
- Use Cow for line normalization (zero-copy for LF-terminated files)
- Cache modal field count in Table struct (eliminates HashMap allocs)
- Return best table from scoring to avoid redundant parsing (saves 2 parses)
- Fix O(n²) preamble detection with suffix count precomputation

Results are now deterministic (v0.2.x had non-deterministic tie-breaking).

| Dataset | v0.2.x* | v0.3.0 | Change |
|:--------|:--------|:-------|:-------|
| POLLOCK | 95.95% | 96.62% | +0.67% |
| W3C-CSVW | 94.12% | 93.21% | -0.91% |
| CSV Wrangling | 91.06% | 91.06% | — |
| CSV Wrangling CODEC | 90.85% | 90.85% | — |
| CSV Wrangling MESSY | 89.68% | 89.68% | — |

*v0.2.x results varied between runs due to non-deterministic HashMap iteration

**Full Changelog**: https://github.com/jqnatividad/csv-nose/compare/v0.2.1...v0.3.0

## [0.2.1] - 2025-01-19

### Added

- added CI test workflow
- registered crate on Zenodo
- added README.md badges

### Changed

- Updated CLAUDE.md with preamble detection documentation

### Performance

No change in detection accuracy.

**Full Changelog**: https://github.com/jqnatividad/csv-nose/compare/v0.2.0...v0.2.1

## [0.2.0] - 2025-01-19

### Added

- Preamble detection for both comment lines (`#`) and structural preambles
  (rows with inconsistent field counts at the start of files)
- `Header.num_preamble_rows` now reports the total preamble count

### Fixed

- Bug where comment preamble count was detected but discarded

### Performance

| Dataset | v0.1.0 | v0.2.0 | Change |
|:--------|:-------|:-------|:-------|
| POLLOCK | 95.95% | 95.95% | — |
| W3C-CSVW | 95.02% | 94.12% | -0.90% |
| CSV Wrangling | 90.50% | 91.06% | +0.56% |
| CSV Wrangling CODEC | 90.14% | 90.85% | +0.71% |
| CSV Wrangling MESSY | 90.48% | 89.68% | -0.80% |

**Full Changelog**: https://github.com/jqnatividad/csv-nose/compare/v0.1.0...v0.2.0

## [0.1.0] - 2025-01-18

### Added

- Initial release implementing the Table Uniformity Method from
  "Detecting CSV File Dialects by Table Uniformity Measurement and Data Type Inference"
  (García, 2024)
- Core dialect detection for delimiter, quote character, and line terminator
- Header row detection using type-based heuristics
- Column type inference (Text, Integer, Float, Boolean, Date, DateTime, Null)
- qsv-sniffer compatible API for drop-in replacement
- CLI tool with multiple output formats (text, JSON)
- Encoding detection and transcoding support (UTF-8, UTF-16, Windows-1251,
  GB2312, ISO-8859, etc.) via chardetng and encoding_rs
- Comment/preamble line detection (lines starting with `#`)
- Benchmark suite against POLLOCK, W3C-CSVW, and CSV Wrangling datasets
- Support for 12 delimiter characters: `,`, `;`, `\t`, `|`, `:`, `~`, `^`,
  `#`, `&`, ` `, `§`, `/`
- Configurable sample size (records, bytes, or entire file)
- Date format preference (MDY vs DMY) for ambiguous dates
- Forced delimiter and quote character options

### Performance

- Zero error rate across all benchmark datasets (no crashes on malformed data)
- ~95% accuracy on POLLOCK dataset
- ~94% accuracy on W3C-CSVW dataset
- ~91% accuracy on CSV Wrangling dataset


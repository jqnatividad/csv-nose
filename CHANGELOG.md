# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.3.1] - 2026-01-20

### Fixed

- `avg_record_len` was always ~1024 bytes regardless of actual record size. Now correctly
  calculates from parsed table data (sum of field lengths plus delimiter and line terminator overhead).

**Full Changelog**: https://github.com/jqnatividad/csv-nose/compare/v0.3.0...v0.3.1

## [0.3.0] - 2026-01-19

### Added

- `PERFORMANCE.md` documenting detection accuracy, known limitations, and workarounds

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


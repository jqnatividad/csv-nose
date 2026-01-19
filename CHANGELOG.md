# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- Preamble detection for both comment lines (`#`) and structural preambles
  (rows with inconsistent field counts at the start of files)
- `Header.num_preamble_rows` now reports the total preamble count

### Fixed
- Bug where comment preamble count was detected but discarded

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

[Unreleased]: https://github.com/jqnatividad/csv-nose/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/jqnatividad/csv-nose/releases/tag/v0.1.0

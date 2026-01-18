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

## License

MIT OR Apache-2.0

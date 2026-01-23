//! csv-nose CLI - CSV dialect sniffer

mod benchmark;
#[cfg(feature = "http")]
mod http;

use benchmark::{find_annotations, run_benchmark};
use clap::Parser;
use csv_nose::{DatePreference, Quote, SampleSize, Sniffer};
use std::path::PathBuf;
use std::process::ExitCode;

/// CSV dialect sniffer using the Table Uniformity Method.
///
/// Detects CSV dialect (delimiter, quote character, header presence)
/// with high accuracy using the Table Uniformity Method.
#[derive(Parser, Debug)]
#[command(name = "csv-nose")]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Input CSV file(s) or URL(s) to sniff, or directory for benchmark mode
    #[arg(required_unless_present = "benchmark")]
    files: Vec<String>,

    /// Run benchmark mode on a directory of test files
    #[arg(long)]
    benchmark: bool,

    /// Path to annotations file for benchmark mode (auto-detected if not specified)
    #[arg(long)]
    annotations: Option<PathBuf>,

    /// Number of records to sample (default: 100)
    #[arg(short = 'n', long, default_value = "100")]
    sample_records: usize,

    /// Number of bytes to sample (overrides --sample-records)
    #[arg(short = 'b', long)]
    sample_bytes: Option<usize>,

    /// Read entire file instead of sampling
    #[arg(short = 'a', long)]
    all: bool,

    /// Force specific delimiter (single character)
    #[arg(short = 'd', long)]
    delimiter: Option<char>,

    /// Force specific quote character (single character, or 'none')
    #[arg(short = 'q', long)]
    quote: Option<String>,

    /// Use day-month-year date format preference (default: month-day-year)
    #[arg(long)]
    dmy: bool,

    /// Output format: text (default), json, or csv
    #[arg(short = 'f', long, default_value = "text")]
    format: OutputFormat,

    /// Show detailed field information
    #[arg(short = 'v', long)]
    verbose: bool,

    /// Only output the detected delimiter character
    #[arg(long)]
    delimiter_only: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
enum OutputFormat {
    Text,
    Json,
    Csv,
}

fn main() -> ExitCode {
    let args = Args::parse();

    // Handle benchmark mode
    if args.benchmark {
        return run_benchmark_cli(&args);
    }

    let mut exit_code = ExitCode::SUCCESS;

    for file in &args.files {
        let result = if is_url(file) {
            #[cfg(feature = "http")]
            {
                sniff_url(file, &args)
            }
            #[cfg(not(feature = "http"))]
            {
                Err("HTTP support not enabled. Rebuild with --features http".into())
            }
        } else {
            sniff_file(&PathBuf::from(file), &args)
        };

        if let Err(e) = result {
            eprintln!("Error processing {file}: {e}");
            exit_code = ExitCode::FAILURE;
        }
    }

    exit_code
}

/// Check if a path looks like a URL.
fn is_url(path: &str) -> bool {
    path.starts_with("http://") || path.starts_with("https://")
}

fn run_benchmark_cli(args: &Args) -> ExitCode {
    if args.files.is_empty() {
        eprintln!("Error: benchmark mode requires a directory path");
        return ExitCode::FAILURE;
    }

    if is_url(&args.files[0]) {
        eprintln!("Error: benchmark mode requires a local directory, not a URL");
        return ExitCode::FAILURE;
    }

    let data_dir = PathBuf::from(&args.files[0]);

    if !data_dir.is_dir() {
        eprintln!("Error: {} is not a directory", data_dir.display());
        return ExitCode::FAILURE;
    }

    // Find or use provided annotations file
    let annotations_path = if let Some(ref path) = args.annotations {
        path.clone()
    } else if let Some(path) = find_annotations(&data_dir) {
        path
    } else {
        eprintln!(
            "Error: Could not find annotations file for {}",
            data_dir.display()
        );
        eprintln!("Use --annotations to specify the path to the annotations file");
        return ExitCode::FAILURE;
    };

    println!("Running benchmark on: {}", data_dir.display());
    println!("Using annotations: {}", annotations_path.display());
    println!();

    match run_benchmark(&data_dir, &annotations_path) {
        Ok(result) => {
            result.print_details();
            result.print_summary();
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("Error running benchmark: {e}");
            ExitCode::FAILURE
        }
    }
}

fn sniff_file(path: &PathBuf, args: &Args) -> Result<(), Box<dyn std::error::Error>> {
    let mut sniffer = Sniffer::new();

    // Configure sample size
    if args.all {
        sniffer.sample_size(SampleSize::All);
    } else if let Some(bytes) = args.sample_bytes {
        sniffer.sample_size(SampleSize::Bytes(bytes));
    } else {
        sniffer.sample_size(SampleSize::Records(args.sample_records));
    }

    // Configure date preference
    if args.dmy {
        sniffer.date_preference(DatePreference::DmyFormat);
    }

    // Configure forced delimiter
    if let Some(delim) = args.delimiter {
        sniffer.delimiter(delim as u8);
    }

    // Configure forced quote
    if let Some(ref quote_str) = args.quote {
        if quote_str.to_lowercase() == "none" {
            sniffer.quote(Quote::None);
        } else if let Some(c) = quote_str.chars().next() {
            sniffer.quote(Quote::Some(c as u8));
        }
    }

    // Sniff the file
    let metadata = sniffer.sniff_path(path)?;

    // Output based on format
    if args.delimiter_only {
        println!("{}", metadata.dialect.delimiter as char);
        return Ok(());
    }

    let display_path = path.display().to_string();
    match args.format {
        OutputFormat::Text => print_text_output(&display_path, &metadata, args.verbose),
        OutputFormat::Json => print_json_output(&display_path, &metadata, args.verbose),
        OutputFormat::Csv => print_csv_output(&display_path, &metadata),
    }

    Ok(())
}

/// Sniff a remote CSV file from a URL using HTTP Range requests.
#[cfg(feature = "http")]
fn sniff_url(url: &str, args: &Args) -> Result<(), Box<dyn std::error::Error>> {
    // Calculate max bytes to fetch
    let max_bytes = if args.all {
        None
    } else if let Some(bytes) = args.sample_bytes {
        Some(bytes)
    } else {
        // For record-based sampling, estimate bytes needed.
        // 500 bytes/record is a reasonable middle ground based on typical CSVs.
        // Users can override with -b/--sample-bytes for specific needs.
        Some(args.sample_records * 500)
    };

    // Fetch data from URL
    let fetch_result = http::fetch_url(url, max_bytes)?;

    let mut sniffer = Sniffer::new();

    // For bytes data, we already limited the fetch, so use SampleSize::All
    sniffer.sample_size(SampleSize::All);

    // Configure date preference
    if args.dmy {
        sniffer.date_preference(DatePreference::DmyFormat);
    }

    // Configure forced delimiter
    if let Some(delim) = args.delimiter {
        sniffer.delimiter(delim as u8);
    }

    // Configure forced quote
    if let Some(ref quote_str) = args.quote {
        if quote_str.to_lowercase() == "none" {
            sniffer.quote(Quote::None);
        } else if let Some(c) = quote_str.chars().next() {
            sniffer.quote(Quote::Some(c as u8));
        }
    }

    // Sniff the fetched bytes
    let metadata = sniffer.sniff_bytes(&fetch_result.data)?;

    // Output based on format
    if args.delimiter_only {
        println!("{}", metadata.dialect.delimiter as char);
        return Ok(());
    }

    match args.format {
        OutputFormat::Text => print_text_output(url, &metadata, args.verbose),
        OutputFormat::Json => print_json_output(url, &metadata, args.verbose),
        OutputFormat::Csv => print_csv_output(url, &metadata),
    }

    Ok(())
}

fn print_text_output(path: &str, metadata: &csv_nose::Metadata, verbose: bool) {
    println!("File: {path}");
    println!("  Delimiter: {:?}", metadata.dialect.delimiter as char);
    println!(
        "  Quote: {}",
        match metadata.dialect.quote {
            Quote::None => "none".to_string(),
            Quote::Some(q) => format!("{:?}", q as char),
        }
    );
    println!("  Has header: {}", metadata.dialect.header.has_header_row);
    println!(
        "  Preamble rows: {}",
        metadata.dialect.header.num_preamble_rows
    );
    println!("  Flexible: {}", metadata.dialect.flexible);
    println!("  UTF-8: {}", metadata.dialect.is_utf8);
    println!("  Fields: {}", metadata.num_fields);
    println!("  Avg record length: {} bytes", metadata.avg_record_len);

    if verbose {
        println!("  Field details:");
        for (i, (name, typ)) in metadata
            .fields
            .iter()
            .zip(metadata.types.iter())
            .enumerate()
        {
            println!("    {}: {} ({})", i + 1, name, typ);
        }
    }

    println!();
}

/// Escape a string for JSON output (handles quotes, backslashes, and control characters).
fn escape_json(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '"' => result.push_str("\\\""),
            '\\' => result.push_str("\\\\"),
            '\n' => result.push_str("\\n"),
            '\r' => result.push_str("\\r"),
            '\t' => result.push_str("\\t"),
            c if c.is_control() => {
                result.push_str(&format!("\\u{:04x}", c as u32));
            }
            c => result.push(c),
        }
    }
    result
}

/// Escape a string for CSV output (quotes the value and doubles internal quotes).
fn escape_csv(s: &str) -> String {
    if s.contains(',') || s.contains('"') || s.contains('\n') || s.contains('\r') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

fn print_json_output(path: &str, metadata: &csv_nose::Metadata, verbose: bool) {
    let quote_str = match metadata.dialect.quote {
        Quote::None => "null".to_string(),
        Quote::Some(q) => format!("\"{}\"", q as char),
    };

    print!(
        r#"{{"file":"{}","dialect":{{"delimiter":"{}","quote":{},"has_header":{},"preamble_rows":{},"flexible":{},"is_utf8":{}}},"num_fields":{},"avg_record_len":{}"#,
        escape_json(path),
        metadata.dialect.delimiter as char,
        quote_str,
        metadata.dialect.header.has_header_row,
        metadata.dialect.header.num_preamble_rows,
        metadata.dialect.flexible,
        metadata.dialect.is_utf8,
        metadata.num_fields,
        metadata.avg_record_len
    );

    if verbose {
        print!(r#","fields":["#);
        for (i, (name, typ)) in metadata
            .fields
            .iter()
            .zip(metadata.types.iter())
            .enumerate()
        {
            if i > 0 {
                print!(",");
            }
            print!(
                r#"{{"name":"{}","type":"{}"}}"#,
                escape_json(name),
                escape_json(&typ.to_string())
            );
        }
        print!("]");
    }

    println!("}}");
}

fn print_csv_output(path: &str, metadata: &csv_nose::Metadata) {
    static mut HEADER_PRINTED: bool = false;

    let quote_str = match metadata.dialect.quote {
        Quote::None => "none".to_string(),
        Quote::Some(q) => format!("{}", q as char),
    };

    // CSV header (print only for first file or could be configured)
    unsafe {
        if !HEADER_PRINTED {
            println!(
                "file,delimiter,quote,has_header,preamble_rows,flexible,is_utf8,num_fields,avg_record_len"
            );
            HEADER_PRINTED = true;
        }
    }

    println!(
        "{},{},{},{},{},{},{},{},{}",
        escape_csv(path),
        metadata.dialect.delimiter as char,
        quote_str,
        metadata.dialect.header.has_header_row,
        metadata.dialect.header.num_preamble_rows,
        metadata.dialect.flexible,
        metadata.dialect.is_utf8,
        metadata.num_fields,
        metadata.avg_record_len
    );
}

//! csv-nose CLI - CSV dialect sniffer

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
    /// Input CSV file(s) to sniff
    #[arg(required = true)]
    files: Vec<PathBuf>,

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

    let mut exit_code = ExitCode::SUCCESS;

    for file in &args.files {
        if let Err(e) = sniff_file(file, &args) {
            eprintln!("Error processing {}: {}", file.display(), e);
            exit_code = ExitCode::FAILURE;
        }
    }

    exit_code
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

    match args.format {
        OutputFormat::Text => print_text_output(path, &metadata, args.verbose),
        OutputFormat::Json => print_json_output(path, &metadata, args.verbose),
        OutputFormat::Csv => print_csv_output(path, &metadata),
    }

    Ok(())
}

fn print_text_output(path: &PathBuf, metadata: &csv_nose::Metadata, verbose: bool) {
    println!("File: {}", path.display());
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

fn print_json_output(path: &PathBuf, metadata: &csv_nose::Metadata, verbose: bool) {
    let quote_str = match metadata.dialect.quote {
        Quote::None => "null".to_string(),
        Quote::Some(q) => format!("\"{}\"", q as char),
    };

    print!(
        r#"{{"file":"{}","dialect":{{"delimiter":"{}","quote":{},"has_header":{},"preamble_rows":{},"flexible":{},"is_utf8":{}}},"num_fields":{},"avg_record_len":{}"#,
        path.display(),
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
            print!(r#"{{"name":"{}","type":"{}"}}"#, name, typ);
        }
        print!("]");
    }

    println!("}}");
}

fn print_csv_output(path: &PathBuf, metadata: &csv_nose::Metadata) {
    let quote_str = match metadata.dialect.quote {
        Quote::None => "none".to_string(),
        Quote::Some(q) => format!("{}", q as char),
    };

    // CSV header (print only for first file or could be configured)
    static mut HEADER_PRINTED: bool = false;
    unsafe {
        if !HEADER_PRINTED {
            println!("file,delimiter,quote,has_header,preamble_rows,flexible,is_utf8,num_fields,avg_record_len");
            HEADER_PRINTED = true;
        }
    }

    println!(
        "{},{},{},{},{},{},{},{},{}",
        path.display(),
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

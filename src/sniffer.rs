//! Main Sniffer builder and sniff methods.
//!
//! This module provides the qsv-sniffer compatible API.

use std::fs::File;
use std::io::{Read, Seek};
use std::path::Path;

use crate::encoding::{detect_encoding, skip_bom};
use crate::error::{Result, SnifferError};
use crate::field_type::Type;
use crate::metadata::{Dialect, Header, Metadata, Quote};
use crate::sample::{DatePreference, SampleSize};
use crate::tum::potential_dialects::{
    detect_line_terminator, generate_dialects_with_terminator, PotentialDialect,
};
use crate::tum::score::{find_best_dialect, score_all_dialects, DialectScore};
use crate::tum::table::parse_table;
use crate::tum::type_detection::infer_column_types;

/// CSV dialect sniffer using the Table Uniformity Method.
///
/// # Example
///
/// ```no_run
/// use csv_nose::{Sniffer, SampleSize};
///
/// let mut sniffer = Sniffer::new();
/// sniffer.sample_size(SampleSize::Records(100));
///
/// let metadata = sniffer.sniff_path("data.csv").unwrap();
/// println!("Delimiter: {}", metadata.dialect.delimiter as char);
/// println!("Has header: {}", metadata.dialect.header.has_header_row);
/// ```
#[derive(Debug, Clone)]
pub struct Sniffer {
    /// Sample size for sniffing.
    sample_size: SampleSize,
    /// Date format preference for ambiguous dates.
    date_preference: DatePreference,
    /// Optional forced delimiter.
    forced_delimiter: Option<u8>,
    /// Optional forced quote character.
    forced_quote: Option<Quote>,
}

impl Default for Sniffer {
    fn default() -> Self {
        Self::new()
    }
}

impl Sniffer {
    /// Create a new Sniffer with default settings.
    pub fn new() -> Self {
        Self {
            sample_size: SampleSize::Records(100),
            date_preference: DatePreference::MdyFormat,
            forced_delimiter: None,
            forced_quote: None,
        }
    }

    /// Set the sample size for sniffing.
    pub fn sample_size(&mut self, sample_size: SampleSize) -> &mut Self {
        self.sample_size = sample_size;
        self
    }

    /// Set the date preference for ambiguous date parsing.
    pub fn date_preference(&mut self, date_preference: DatePreference) -> &mut Self {
        self.date_preference = date_preference;
        self
    }

    /// Force a specific delimiter (skip delimiter detection).
    pub fn delimiter(&mut self, delimiter: u8) -> &mut Self {
        self.forced_delimiter = Some(delimiter);
        self
    }

    /// Force a specific quote character.
    pub fn quote(&mut self, quote: Quote) -> &mut Self {
        self.forced_quote = Some(quote);
        self
    }

    /// Sniff a CSV file at the given path.
    pub fn sniff_path<P: AsRef<Path>>(&mut self, path: P) -> Result<Metadata> {
        let file = File::open(path.as_ref())?;
        let mut reader = std::io::BufReader::new(file);
        self.sniff_reader(&mut reader)
    }

    /// Sniff CSV data from a reader.
    pub fn sniff_reader<R: Read + Seek>(&mut self, reader: R) -> Result<Metadata> {
        let data = self.read_sample(reader)?;

        if data.is_empty() {
            return Err(SnifferError::EmptyData);
        }

        self.sniff_bytes(&data)
    }

    /// Sniff CSV data from bytes.
    pub fn sniff_bytes(&self, data: &[u8]) -> Result<Metadata> {
        if data.is_empty() {
            return Err(SnifferError::EmptyData);
        }

        // Detect encoding
        let encoding_info = detect_encoding(data);
        let data = skip_bom(data);

        // Detect line terminator first to reduce search space
        let line_terminator = detect_line_terminator(data);

        // Generate potential dialects
        let dialects = if let Some(delim) = self.forced_delimiter {
            // If delimiter is forced, only test that delimiter with different quotes
            let quotes = if let Some(q) = self.forced_quote {
                vec![q]
            } else {
                vec![Quote::Some(b'"'), Quote::Some(b'\''), Quote::None]
            };

            quotes
                .into_iter()
                .map(|q| PotentialDialect::new(delim, q, line_terminator))
                .collect()
        } else {
            generate_dialects_with_terminator(line_terminator)
        };

        // Determine max rows for scoring
        let max_rows = match self.sample_size {
            SampleSize::Records(n) => n,
            SampleSize::Bytes(_) | SampleSize::All => 0, // Already limited by read_sample
        };

        // Score all dialects
        let scores = score_all_dialects(data, &dialects, max_rows);

        // Find the best dialect
        let best = find_best_dialect(&scores)
            .ok_or_else(|| SnifferError::NoDialectDetected("No valid dialect found".to_string()))?;

        // Build metadata from the best dialect
        self.build_metadata(data, best, encoding_info.is_utf8)
    }

    /// Read a sample of data from the reader based on sample_size settings.
    fn read_sample<R: Read + Seek>(&self, mut reader: R) -> Result<Vec<u8>> {
        match self.sample_size {
            SampleSize::Bytes(n) => {
                let mut buffer = vec![0u8; n];
                let bytes_read = reader.read(&mut buffer)?;
                buffer.truncate(bytes_read);
                Ok(buffer)
            }
            SampleSize::All => {
                let mut buffer = Vec::new();
                reader.read_to_end(&mut buffer)?;
                Ok(buffer)
            }
            SampleSize::Records(n) => {
                // For records, we read enough to capture n records
                // Estimate ~1KB per record as a starting point, with a minimum
                let estimated_size = (n * 1024).max(8192);
                let mut buffer = vec![0u8; estimated_size];
                let bytes_read = reader.read(&mut buffer)?;
                buffer.truncate(bytes_read);

                // If we need more data, keep reading
                if bytes_read == estimated_size {
                    // Count newlines to see if we have enough records
                    let newlines = buffer.iter().filter(|&&b| b == b'\n').count();
                    if newlines < n {
                        // Read more data
                        let additional = (n - newlines) * 2048;
                        let mut more = vec![0u8; additional];
                        let more_read = reader.read(&mut more)?;
                        more.truncate(more_read);
                        buffer.extend(more);
                    }
                }

                Ok(buffer)
            }
        }
    }

    /// Build Metadata from the best scoring dialect.
    fn build_metadata(&self, data: &[u8], score: &DialectScore, is_utf8: bool) -> Result<Metadata> {
        // Parse the table with the best dialect
        let max_rows = match self.sample_size {
            SampleSize::Records(n) => n,
            _ => 0,
        };

        let table = parse_table(data, &score.dialect, max_rows);

        if table.is_empty() {
            return Err(SnifferError::EmptyData);
        }

        // Detect header
        let header = detect_header(&table, &score.dialect);

        // Get field names
        let fields = if header.has_header_row && !table.rows.is_empty() {
            table.rows[0].clone()
        } else {
            // Generate field names
            (0..score.num_fields)
                .map(|i| format!("field_{}", i + 1))
                .collect()
        };

        // Skip header row for type inference if present
        let data_table = if header.has_header_row && table.rows.len() > 1 {
            let mut dt = crate::tum::table::Table::new();
            dt.rows = table.rows[1..].to_vec();
            dt.field_counts = table.field_counts[1..].to_vec();
            dt
        } else {
            table.clone()
        };

        // Infer types for each column
        let types = infer_column_types(&data_table);

        // Build dialect
        let dialect = Dialect {
            delimiter: score.dialect.delimiter,
            header,
            quote: score.dialect.quote,
            flexible: !score.is_uniform,
            is_utf8,
        };

        // Calculate average record length
        let avg_record_len = calculate_avg_record_len(data, table.num_rows());

        Ok(Metadata {
            dialect,
            avg_record_len,
            num_fields: score.num_fields,
            fields,
            types,
        })
    }
}

/// Detect if the first row is likely a header row.
fn detect_header(table: &crate::tum::table::Table, _dialect: &PotentialDialect) -> Header {
    if table.rows.is_empty() {
        return Header::new(false, 0);
    }

    if table.rows.len() < 2 {
        // Can't determine header with only one row
        return Header::new(false, 0);
    }

    let first_row = &table.rows[0];
    let second_row = &table.rows[1];

    // Heuristics for header detection:
    // 1. First row has different types than subsequent rows
    // 2. First row values look like labels (text when data is numeric)
    // 3. First row has no duplicates (header columns should be unique)

    let mut header_score = 0.0;
    let mut checks = 0;

    // Check 1: First row is all text, second row has typed data
    let first_types: Vec<Type> = first_row
        .iter()
        .map(|s| crate::tum::type_detection::detect_cell_type(s))
        .collect();
    let second_types: Vec<Type> = second_row
        .iter()
        .map(|s| crate::tum::type_detection::detect_cell_type(s))
        .collect();

    let first_text_count = first_types.iter().filter(|&&t| t == Type::Text).count();
    let second_text_count = second_types.iter().filter(|&&t| t == Type::Text).count();

    if first_text_count > second_text_count {
        header_score += 1.0;
    }
    checks += 1;

    // Check 2: First row has more text than numeric
    let first_numeric_count = first_types.iter().filter(|&&t| t.is_numeric()).count();
    if first_text_count > first_numeric_count {
        header_score += 0.5;
    }
    checks += 1;

    // Check 3: No duplicates in first row
    let unique_count = {
        let mut seen = std::collections::HashSet::new();
        first_row.iter().filter(|s| seen.insert(s.as_str())).count()
    };
    if unique_count == first_row.len() {
        header_score += 0.5;
    }
    checks += 1;

    // Check 4: First row values are shorter (headers tend to be concise)
    let avg_first_len: f64 =
        first_row.iter().map(|s| s.len()).sum::<usize>() as f64 / first_row.len().max(1) as f64;
    let avg_second_len: f64 =
        second_row.iter().map(|s| s.len()).sum::<usize>() as f64 / second_row.len().max(1) as f64;

    if avg_first_len <= avg_second_len {
        header_score += 0.3;
    }
    checks += 1;

    // Threshold for header detection
    let has_header = (header_score / checks as f64) > 0.4;

    Header::new(has_header, 0)
}

/// Calculate average record length.
fn calculate_avg_record_len(data: &[u8], num_rows: usize) -> usize {
    if num_rows == 0 {
        return 0;
    }
    data.len() / num_rows
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sniffer_builder() {
        let mut sniffer = Sniffer::new();
        sniffer
            .sample_size(SampleSize::Records(50))
            .date_preference(DatePreference::DmyFormat)
            .delimiter(b',');

        assert_eq!(sniffer.sample_size, SampleSize::Records(50));
        assert_eq!(sniffer.date_preference, DatePreference::DmyFormat);
        assert_eq!(sniffer.forced_delimiter, Some(b','));
    }

    #[test]
    fn test_sniff_bytes() {
        let data = b"name,age,city\nAlice,30,NYC\nBob,25,LA\n";
        let sniffer = Sniffer::new();

        let metadata = sniffer.sniff_bytes(data).unwrap();

        assert_eq!(metadata.dialect.delimiter, b',');
        assert!(metadata.dialect.header.has_header_row);
        assert_eq!(metadata.num_fields, 3);
        assert_eq!(metadata.fields, vec!["name", "age", "city"]);
    }

    #[test]
    fn test_sniff_tsv() {
        let data = b"name\tage\tcity\nAlice\t30\tNYC\nBob\t25\tLA\n";
        let sniffer = Sniffer::new();

        let metadata = sniffer.sniff_bytes(data).unwrap();

        assert_eq!(metadata.dialect.delimiter, b'\t');
        assert!(metadata.dialect.header.has_header_row);
    }

    #[test]
    fn test_sniff_semicolon() {
        let data = b"name;age;city\nAlice;30;NYC\nBob;25;LA\n";
        let sniffer = Sniffer::new();

        let metadata = sniffer.sniff_bytes(data).unwrap();

        assert_eq!(metadata.dialect.delimiter, b';');
    }

    #[test]
    fn test_sniff_no_header() {
        let data = b"1,2,3\n4,5,6\n7,8,9\n";
        let sniffer = Sniffer::new();

        let metadata = sniffer.sniff_bytes(data).unwrap();

        assert_eq!(metadata.dialect.delimiter, b',');
        // All numeric data - should not detect header
        assert!(!metadata.dialect.header.has_header_row);
    }

    #[test]
    fn test_sniff_with_quotes() {
        let data = b"\"name\",\"value\"\n\"hello, world\",123\n\"test\",456\n";
        let sniffer = Sniffer::new();

        let metadata = sniffer.sniff_bytes(data).unwrap();

        assert_eq!(metadata.dialect.delimiter, b',');
        assert_eq!(metadata.dialect.quote, Quote::Some(b'"'));
    }

    #[test]
    fn test_sniff_empty() {
        let data = b"";
        let sniffer = Sniffer::new();

        let result = sniffer.sniff_bytes(data);
        assert!(result.is_err());
    }
}

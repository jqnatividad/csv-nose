//! Main Sniffer builder and sniff methods.
//!
//! This module provides the qsv-sniffer compatible API.

use std::fs::File;
use std::io::{Read, Seek};
use std::path::Path;

use crate::encoding::{detect_and_transcode, detect_encoding, skip_bom};
use crate::error::{Result, SnifferError};
use crate::field_type::Type;
use crate::metadata::{Dialect, Header, Metadata, Quote};
use crate::sample::{DatePreference, SampleSize};
use crate::tum::potential_dialects::{
    PotentialDialect, detect_line_terminator, generate_dialects_with_terminator,
};
use crate::tum::score::{DialectScore, find_best_dialect, score_all_dialects_with_best_table};
use crate::tum::table::{Table, parse_table};
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
    pub const fn new() -> Self {
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

        // Detect encoding and transcode to UTF-8 if necessary
        let (transcoded_data, was_transcoded) = detect_and_transcode(data);
        let data = &transcoded_data[..];

        // Detect encoding info (for metadata)
        let encoding_info = detect_encoding(data);
        let is_utf8 = !was_transcoded || encoding_info.is_utf8;

        // Skip BOM
        let data = skip_bom(data);

        // Skip comment/preamble lines (lines starting with #)
        let (comment_preamble_rows, data) = skip_preamble(data);

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

        // Score all dialects and get the best table (avoids re-parsing)
        let (scores, best_table) = score_all_dialects_with_best_table(data, &dialects, max_rows);

        // Find the best dialect
        let best = find_best_dialect(&scores)
            .ok_or_else(|| SnifferError::NoDialectDetected("No valid dialect found".to_string()))?;

        // Detect structural preamble using the already-parsed table
        let table_for_preamble =
            best_table.unwrap_or_else(|| parse_table(data, &best.dialect, max_rows));
        let structural_preamble = detect_structural_preamble(&table_for_preamble);

        // Total preamble = comment rows + structural rows
        let total_preamble_rows = comment_preamble_rows + structural_preamble;

        // Build metadata from the best dialect, reusing the already-parsed table
        // Pass structural_preamble for table row indexing (since comment rows are already skipped from data)
        // Pass total_preamble_rows for Header metadata (to report true preamble count in original file)
        self.build_metadata(
            best,
            is_utf8,
            structural_preamble,
            total_preamble_rows,
            table_for_preamble,
        )
    }

    /// Read a sample of data from the reader based on `sample_size` settings.
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
                    let newlines = bytecount::count(&buffer, b'\n');
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
    ///
    /// # Arguments
    /// * `structural_preamble` - Number of structural preamble rows in the table (for row indexing)
    /// * `total_preamble_rows` - Total preamble rows including comments (for Header metadata)
    /// * `table` - Pre-parsed table to avoid redundant parsing
    fn build_metadata(
        &self,
        score: &DialectScore,
        is_utf8: bool,
        structural_preamble: usize,
        total_preamble_rows: usize,
        table: Table,
    ) -> Result<Metadata> {
        if table.is_empty() {
            return Err(SnifferError::EmptyData);
        }

        // Create a view of the table without structural preamble
        // (comment preamble rows are already stripped from data)
        let effective_table = if structural_preamble > 0 && table.rows.len() > structural_preamble {
            let mut et = crate::tum::table::Table::new();
            et.rows = table.rows[structural_preamble..].to_vec();
            et.field_counts = table.field_counts[structural_preamble..].to_vec();
            et.update_modal_field_count();
            et
        } else {
            table.clone()
        };

        // Detect header on the effective table (pass total_preamble_rows for Header metadata)
        let header = detect_header(&effective_table, &score.dialect, total_preamble_rows);

        // Get field names from the effective table (first row after structural preamble)
        let fields = if header.has_header_row && !effective_table.rows.is_empty() {
            effective_table.rows[0].clone()
        } else {
            // Generate field names
            (0..score.num_fields)
                .map(|i| format!("field_{}", i + 1))
                .collect()
        };

        // Skip header row for type inference if present
        let data_table = if header.has_header_row && effective_table.rows.len() > 1 {
            let mut dt = crate::tum::table::Table::new();
            dt.rows = effective_table.rows[1..].to_vec();
            dt.field_counts = effective_table.field_counts[1..].to_vec();
            dt.update_modal_field_count();
            dt
        } else {
            effective_table
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

        // Calculate average record length from the parsed table
        let avg_record_len = calculate_avg_record_len(&table);

        Ok(Metadata {
            dialect,
            avg_record_len,
            num_fields: score.num_fields,
            fields,
            types,
        })
    }
}

/// Detect if the first row (after preamble) is likely a header row.
fn detect_header(
    table: &crate::tum::table::Table,
    _dialect: &PotentialDialect,
    preamble_rows: usize,
) -> Header {
    if table.rows.is_empty() {
        return Header::new(false, preamble_rows);
    }

    if table.rows.len() < 2 {
        // Can't determine header with only one row
        return Header::new(false, preamble_rows);
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
    let avg_first_len: f64 = first_row
        .iter()
        .map(std::string::String::len)
        .sum::<usize>() as f64
        / first_row.len().max(1) as f64;
    let avg_second_len: f64 = second_row
        .iter()
        .map(std::string::String::len)
        .sum::<usize>() as f64
        / second_row.len().max(1) as f64;

    if avg_first_len <= avg_second_len {
        header_score += 0.3;
    }
    checks += 1;

    // Threshold for header detection
    let has_header = (header_score / checks as f64) > 0.4;

    Header::new(has_header, preamble_rows)
}

/// Calculate average record length from the parsed table.
///
/// Calculates based on actual parsed content: sum of field lengths plus
/// delimiters and line terminator overhead.
fn calculate_avg_record_len(table: &crate::tum::table::Table) -> usize {
    if table.num_rows() == 0 {
        return 0;
    }

    let total_len: usize = table
        .rows
        .iter()
        .map(|row| {
            // Sum of all field lengths
            let field_len: usize = row.iter().map(String::len).sum();
            // Add delimiter overhead (one less than number of fields, each delimiter is 1 byte)
            let delimiter_overhead = row.len().saturating_sub(1);
            // Add ~2 bytes for line terminator (average of \n and \r\n)
            field_len + delimiter_overhead + 2
        })
        .sum();

    total_len / table.num_rows()
}

/// Skip preamble/comment lines at the start of data.
///
/// Detects lines starting with '#' at the beginning of the file and returns
/// the number of preamble rows and a slice starting after the preamble.
fn skip_preamble(data: &[u8]) -> (usize, &[u8]) {
    let mut preamble_rows = 0;
    let mut offset = 0;

    while offset < data.len() {
        // Skip leading whitespace on the line
        let mut line_start = offset;
        while line_start < data.len() && (data[line_start] == b' ' || data[line_start] == b'\t') {
            line_start += 1;
        }

        // Check if line starts with #
        if line_start < data.len() && data[line_start] == b'#' {
            // Find end of line
            let mut line_end = line_start;
            while line_end < data.len() && data[line_end] != b'\n' && data[line_end] != b'\r' {
                line_end += 1;
            }

            // Skip line terminator
            if line_end < data.len() && data[line_end] == b'\r' {
                line_end += 1;
            }
            if line_end < data.len() && data[line_end] == b'\n' {
                line_end += 1;
            }

            preamble_rows += 1;
            offset = line_end;
        } else {
            // Not a comment line, stop
            break;
        }
    }

    (preamble_rows, &data[offset..])
}

/// Detect structural preamble rows using field count consistency analysis.
///
/// Identifies rows at the start that don't match the predominant field count
/// pattern (metadata rows, empty rows, title rows with different structure).
fn detect_structural_preamble(table: &crate::tum::table::Table) -> usize {
    let n = table.field_counts.len();
    if n < 3 {
        return 0;
    }

    let modal_count = table.modal_field_count();

    // Pre-compute suffix counts: for each position i, how many rows from i to end match modal_count
    // This converts O(n²) scanning to O(n) preprocessing + O(1) lookups
    let mut matching_suffix = vec![0usize; n];
    let mut count = 0;
    for i in (0..n).rev() {
        if table.field_counts[i] == modal_count {
            count += 1;
        }
        matching_suffix[i] = count;
    }

    // Find first row where remaining data is 80%+ consistent with modal field count
    for (i, &field_count) in table.field_counts.iter().enumerate() {
        if field_count == modal_count {
            let remaining_len = n - i;
            let matching = matching_suffix[i];
            let consistency = matching as f64 / remaining_len as f64;

            if consistency >= 0.8 {
                return i;
            }
        }
    }

    0
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

    #[test]
    fn test_skip_preamble() {
        // Test with comment lines
        let data = b"# This is a comment\n# Another comment\nname,age\nAlice,30\n";
        let (preamble_rows, remaining) = skip_preamble(data);
        assert_eq!(preamble_rows, 2);
        assert_eq!(remaining, b"name,age\nAlice,30\n");

        // Test without comment lines
        let data = b"name,age\nAlice,30\n";
        let (preamble_rows, remaining) = skip_preamble(data);
        assert_eq!(preamble_rows, 0);
        assert_eq!(remaining, b"name,age\nAlice,30\n");

        // Test with whitespace before #
        let data = b"  # Indented comment\nname,age\n";
        let (preamble_rows, remaining) = skip_preamble(data);
        assert_eq!(preamble_rows, 1);
        assert_eq!(remaining, b"name,age\n");
    }

    #[test]
    fn test_sniff_with_preamble() {
        let data = b"# LimeSurvey export\n# Generated 2024-01-01\nname,age,city\nAlice,30,NYC\nBob,25,LA\n";
        let sniffer = Sniffer::new();

        let metadata = sniffer.sniff_bytes(data).unwrap();

        assert_eq!(metadata.dialect.delimiter, b',');
        assert!(metadata.dialect.header.has_header_row);
        assert_eq!(metadata.num_fields, 3);
    }

    #[test]
    fn test_comment_preamble_propagated() {
        let data = b"# Comment 1\n# Comment 2\nname,age\nAlice,30\nBob,25\n";
        let metadata = Sniffer::new().sniff_bytes(data).unwrap();
        assert_eq!(metadata.dialect.header.num_preamble_rows, 2);
        assert!(metadata.dialect.header.has_header_row);
        assert_eq!(metadata.fields, vec!["name", "age"]);
    }

    #[test]
    fn test_structural_preamble_detection() {
        // TITLE row has 1 field, SUBTITLE has 2 fields, data has 5 fields
        let data = b"TITLE\nSUB,TITLE\nA,B,C,D,E\n1,2,3,4,5\n2,3,4,5,6\n3,4,5,6,7\n";
        let metadata = Sniffer::new().sniff_bytes(data).unwrap();
        assert_eq!(metadata.dialect.header.num_preamble_rows, 2);
        assert!(metadata.dialect.header.has_header_row);
        assert_eq!(metadata.fields, vec!["A", "B", "C", "D", "E"]);
    }

    #[test]
    fn test_mixed_preamble_detection() {
        // Both comment preamble and structural preamble
        // METADATA has 1 field, data has 3 fields
        let data =
            b"# File header\nMETADATA\nname,age,city\nAlice,30,NYC\nBob,25,LA\nCharlie,35,CHI\n";
        let metadata = Sniffer::new().sniff_bytes(data).unwrap();
        // 1 comment + 1 structural = 2 total
        assert_eq!(metadata.dialect.header.num_preamble_rows, 2);
        assert!(metadata.dialect.header.has_header_row);
        assert_eq!(metadata.fields, vec!["name", "age", "city"]);
    }

    #[test]
    fn test_no_preamble() {
        let data = b"a,b,c\n1,2,3\n4,5,6\n";
        let metadata = Sniffer::new().sniff_bytes(data).unwrap();
        assert_eq!(metadata.dialect.header.num_preamble_rows, 0);
    }

    #[test]
    fn test_detect_structural_preamble_function() {
        use crate::tum::table::Table;

        // Table with 2 preamble rows (different field counts)
        let mut table = Table::new();
        table.rows = vec![
            vec!["TITLE".to_string()],
            vec!["".to_string(), "".to_string()],
            vec!["A".to_string(), "B".to_string(), "C".to_string()],
            vec!["1".to_string(), "2".to_string(), "3".to_string()],
            vec!["4".to_string(), "5".to_string(), "6".to_string()],
        ];
        table.field_counts = vec![1, 2, 3, 3, 3];
        table.update_modal_field_count();
        assert_eq!(detect_structural_preamble(&table), 2);

        // Table with no preamble (uniform field counts)
        let mut table = Table::new();
        table.rows = vec![
            vec!["A".to_string(), "B".to_string(), "C".to_string()],
            vec!["1".to_string(), "2".to_string(), "3".to_string()],
        ];
        table.field_counts = vec![3, 3];
        table.update_modal_field_count();
        assert_eq!(detect_structural_preamble(&table), 0);

        // Table too small to determine preamble
        let mut table = Table::new();
        table.rows = vec![vec!["A".to_string()]];
        table.field_counts = vec![1];
        table.update_modal_field_count();
        assert_eq!(detect_structural_preamble(&table), 0);
    }

    #[test]
    fn test_avg_record_len_calculated_from_data() {
        // Test that avg_record_len is calculated from actual data, not hardcoded
        let short_data = b"a,b\n1,2\n3,4\n";
        let sniffer = Sniffer::new();
        let metadata = sniffer.sniff_bytes(short_data).unwrap();

        // Each row: 2 fields of 1 char each + 1 delimiter + 2 line terminator estimate = 5 bytes
        // Should be small, definitely NOT the old hardcoded 1024
        assert!(
            metadata.avg_record_len < 100,
            "avg_record_len should be small for short records, got {}",
            metadata.avg_record_len
        );

        // Test with longer fields to verify it scales with actual content
        let long_data =
            b"very_long_field_name,another_long_field_name\nvalue1,value2\nval3,val4\n";
        let metadata_long = sniffer.sniff_bytes(long_data).unwrap();

        // Longer fields should result in larger avg_record_len
        assert!(
            metadata_long.avg_record_len > metadata.avg_record_len,
            "longer fields should have larger avg_record_len: short={}, long={}",
            metadata.avg_record_len,
            metadata_long.avg_record_len
        );
    }
}

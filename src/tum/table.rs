//! CSV parsing into a table structure for analysis.

use super::potential_dialects::PotentialDialect;
use crate::metadata::Quote;
use foldhash::{HashMap, HashMapExt};
use std::borrow::Cow;
use std::io::{BufRead, Cursor};

/// A parsed CSV table for analysis.
#[derive(Debug, Clone)]
pub struct Table {
    /// The rows of the table (each row is a vector of field values).
    pub rows: Vec<Vec<String>>,
    /// Number of fields in each row.
    pub field_counts: Vec<usize>,
    /// Cached modal (most common) field count, computed during parsing.
    cached_modal_field_count: usize,
}

impl Table {
    /// Create a new empty table.
    pub const fn new() -> Self {
        Self {
            rows: Vec::new(),
            field_counts: Vec::new(),
            cached_modal_field_count: 0,
        }
    }

    /// Returns true if the table is empty.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    /// Returns the number of rows.
    #[inline]
    pub fn num_rows(&self) -> usize {
        self.rows.len()
    }

    /// Returns the modal (most common) field count.
    /// Uses cached value computed during parsing for efficiency.
    #[inline]

    pub fn modal_field_count(&self) -> usize {
        self.cached_modal_field_count
    }

    /// Compute the modal field count from field_counts.
    /// Called internally after parsing or when constructing tables manually.
    fn compute_modal_field_count(field_counts: &[usize]) -> usize {
        if field_counts.is_empty() {
            return 0;
        }

        let mut counts: HashMap<usize, usize> = HashMap::with_capacity(field_counts.len());
        for &fc in field_counts {
            *counts.entry(fc).or_insert(0) += 1;
        }

        // Use deterministic tie-breaking: prefer higher field count when frequencies are equal
        // This ensures consistent results regardless of HashMap iteration order
        counts
            .into_iter()
            .max_by(|(fc_a, count_a), (fc_b, count_b)| {
                count_a.cmp(count_b).then_with(|| fc_a.cmp(fc_b))
            })
            .map_or(0, |(fc, _)| fc)
    }

    /// Update the cached modal field count. Call after modifying field_counts.
    pub fn update_modal_field_count(&mut self) {
        self.cached_modal_field_count = Self::compute_modal_field_count(&self.field_counts);
    }

    /// Returns the minimum field count.
    #[inline]
    pub fn min_field_count(&self) -> usize {
        self.field_counts.iter().copied().min().unwrap_or(0)
    }

    /// Returns the maximum field count.
    #[inline]
    pub fn max_field_count(&self) -> usize {
        self.field_counts.iter().copied().max().unwrap_or(0)
    }
}

impl Default for Table {
    fn default() -> Self {
        Self::new()
    }
}

/// Parse data into a table using the given dialect.
///
/// # Arguments
/// * `data` - The raw CSV data bytes
/// * `dialect` - The dialect to use for parsing
/// * `max_rows` - Maximum number of rows to parse (0 = unlimited)
pub fn parse_table(data: &[u8], dialect: &PotentialDialect, max_rows: usize) -> Table {
    let mut table = Table::new();

    // Build CSV reader with the dialect settings
    let mut reader_builder = csv::ReaderBuilder::new();
    reader_builder
        .delimiter(dialect.delimiter)
        .has_headers(false)
        .flexible(true);

    // Configure quoting
    match dialect.quote {
        Quote::None => {
            reader_builder.quoting(false);
        }
        Quote::Some(q) => {
            reader_builder.quoting(true);
            reader_builder.quote(q);
        }
    }

    // Handle different line terminators by normalizing the data
    let normalized = normalize_line_endings(data, dialect);

    let cursor = Cursor::new(normalized);
    let mut reader = reader_builder.from_reader(cursor);

    let mut record = csv::StringRecord::new();
    let limit = if max_rows == 0 { usize::MAX } else { max_rows };

    while table.rows.len() < limit {
        match reader.read_record(&mut record) {
            Ok(true) => {
                let row: Vec<String> = record
                    .iter()
                    .map(std::string::ToString::to_string)
                    .collect();
                let field_count = row.len();
                table.rows.push(row);
                table.field_counts.push(field_count);
            }
            Ok(false) => break, // EOF
            Err(_) => break,    // Parse error, stop here
        }
    }

    // Cache the modal field count for efficient repeated access
    table.update_modal_field_count();

    table
}

/// Normalize line endings to LF for consistent parsing.
/// Returns `Cow::Borrowed` for LF data (zero-copy) and `Cow::Owned` for CR/CRLF.
fn normalize_line_endings<'a>(data: &'a [u8], dialect: &PotentialDialect) -> Cow<'a, [u8]> {
    use super::potential_dialects::LineTerminator;

    match dialect.line_terminator {
        LineTerminator::LF => Cow::Borrowed(data), // Zero-copy for LF
        LineTerminator::CRLF => {
            // Replace \r\n with \n
            let mut result = Vec::with_capacity(data.len());
            let mut i = 0;
            while i < data.len() {
                if i + 1 < data.len() && data[i] == b'\r' && data[i + 1] == b'\n' {
                    result.push(b'\n');
                    i += 2;
                } else {
                    result.push(data[i]);
                    i += 1;
                }
            }
            Cow::Owned(result)
        }
        LineTerminator::CR => {
            // Replace standalone \r with \n
            Cow::Owned(
                data.iter()
                    .map(|&b| if b == b'\r' { b'\n' } else { b })
                    .collect(),
            )
        }
    }
}

/// Simple line-based parser for when csv crate fails.
///
/// This is a fallback parser that handles edge cases the csv crate might reject.
#[allow(dead_code)]
pub fn parse_table_simple(data: &[u8], dialect: &PotentialDialect, max_rows: usize) -> Table {
    let mut table = Table::new();
    let normalized = normalize_line_endings(data, dialect);

    let cursor = Cursor::new(normalized.as_ref());
    let limit = if max_rows == 0 { usize::MAX } else { max_rows };

    for line in cursor.lines().take(limit) {
        let Ok(line) = line else { continue };
        if line.is_empty() {
            continue;
        }

        let fields = split_line(&line, dialect);
        let field_count = fields.len();
        table.rows.push(fields);
        table.field_counts.push(field_count);
    }

    // Cache the modal field count for efficient repeated access
    table.update_modal_field_count();

    table
}

/// Split a line into fields based on the dialect.
#[allow(dead_code)]
fn split_line(line: &str, dialect: &PotentialDialect) -> Vec<String> {
    let delimiter = dialect.delimiter as char;
    let quote_char = match dialect.quote {
        Quote::None => None,
        Quote::Some(q) => Some(q as char),
    };

    let mut fields = Vec::new();
    let mut current_field = String::new();
    let mut in_quotes = false;
    let mut chars = line.chars().peekable();

    while let Some(c) = chars.next() {
        if let Some(q) = quote_char
            && c == q
        {
            if in_quotes {
                // Check for escaped quote (doubled quote)
                if chars.peek() == Some(&q) {
                    current_field.push(q);
                    chars.next();
                } else {
                    in_quotes = false;
                }
            } else {
                in_quotes = true;
            }
            continue;
        }

        if c == delimiter && !in_quotes {
            fields.push(current_field.trim().to_string());
            current_field = String::new();
        } else {
            current_field.push(c);
        }
    }

    fields.push(current_field.trim().to_string());
    fields
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tum::potential_dialects::LineTerminator;

    #[test]
    fn test_parse_simple_csv() {
        let data = b"a,b,c\n1,2,3\n4,5,6\n";
        let dialect = PotentialDialect::new(b',', Quote::Some(b'"'), LineTerminator::LF);

        let table = parse_table(data, &dialect, 0);
        assert_eq!(table.num_rows(), 3);
        assert_eq!(table.field_counts, vec![3, 3, 3]);
        assert_eq!(table.rows[0], vec!["a", "b", "c"]);
    }

    #[test]
    fn test_parse_quoted_csv() {
        let data = b"\"a,b\",c,d\n1,2,3\n";
        let dialect = PotentialDialect::new(b',', Quote::Some(b'"'), LineTerminator::LF);

        let table = parse_table(data, &dialect, 0);
        assert_eq!(table.num_rows(), 2);
        assert_eq!(table.rows[0], vec!["a,b", "c", "d"]);
    }

    #[test]
    fn test_modal_field_count() {
        let mut table = Table::new();
        table.field_counts = vec![3, 3, 3, 4, 3];
        table.update_modal_field_count();
        assert_eq!(table.modal_field_count(), 3);
    }
}

//! Type detection for CSV cells using optimized string operations.

use super::regexes::*;
use super::table::Table;
use crate::field_type::Type;

/// Check for NULL-like values using string matching instead of regex.
/// This is a hot path optimization - called for every cell.
#[inline]
fn is_null_value(s: &str) -> bool {
    matches!(
        s,
        "" | "-"
            | "--"
            | "."
            | ".."
            | "?"
            | "null"
            | "NULL"
            | "Null"
            | "nil"
            | "NIL"
            | "Nil"
            | "none"
            | "NONE"
            | "None"
            | "na"
            | "NA"
            | "Na"
            | "n/a"
            | "N/A"
            | "N/a"
            | "nan"
            | "NaN"
            | "NAN"
            | "#N/A"
            | "#VALUE!"
            | "#REF!"
            | "#DIV/0!"
    )
}

/// Check for unsigned integer using string parsing instead of regex.
/// This is a hot path optimization - called for every cell.
/// Limit to 19 digits to ensure all values fit in u64 (max is 18,446,744,073,709,551,615).
#[inline]
fn is_unsigned_int(s: &str) -> bool {
    let s = s.strip_prefix('+').unwrap_or(s);
    !s.is_empty() && s.len() <= 19 && s.bytes().all(|b| b.is_ascii_digit())
}

/// Check for signed integer using string parsing instead of regex.
/// Returns true only for negative integers (positive ones are unsigned).
/// Limit to 19 digits to ensure all values fit in i64 (min is -9,223,372,036,854,775,808).
#[inline]
fn is_signed_int(s: &str) -> bool {
    if let Some(rest) = s.strip_prefix('-') {
        !rest.is_empty() && rest.len() <= 19 && rest.bytes().all(|b| b.is_ascii_digit())
    } else {
        false
    }
}

/// Check for boolean values using exhaustive match instead of regex.
/// This is a hot path optimization - called for every cell.
#[inline]
fn is_boolean(s: &str) -> bool {
    match s.len() {
        1 => {
            let b = s.as_bytes()[0].to_ascii_lowercase();
            matches!(b, b'1' | b'0' | b'y' | b'n' | b't' | b'f')
        }
        2 => s.eq_ignore_ascii_case("on") || s.eq_ignore_ascii_case("no"),
        3 => s.eq_ignore_ascii_case("yes") || s.eq_ignore_ascii_case("off"),
        4 => s.eq_ignore_ascii_case("true"),
        5 => s.eq_ignore_ascii_case("false"),
        _ => false,
    }
}

/// Detect the type of a single cell value.
pub fn detect_cell_type(value: &str) -> Type {
    let trimmed = value.trim();

    // Check for empty first
    if trimmed.is_empty() {
        return Type::NULL;
    }

    // Check for NULL-like values using optimized string matching
    if is_null_value(trimmed) {
        return Type::NULL;
    }

    // Check for unsigned integer (must come before boolean since 1/0 match boolean)
    if is_unsigned_int(trimmed) {
        return Type::Unsigned;
    }

    // Check for signed integer (negative numbers only)
    if is_signed_int(trimmed) {
        return Type::Signed;
    }

    // Check for boolean (after integers so "1" and "0" are treated as numbers)
    if is_boolean(trimmed) {
        return Type::Boolean;
    }

    // Check for float - use regex for complex patterns but fast-path simple cases
    if FLOAT_PATTERN.is_match(trimmed) {
        // Distinguish between integer-like floats and actual floats
        // Avoid to_lowercase() allocation by checking both cases directly
        if trimmed.contains('.') || trimmed.contains('e') || trimmed.contains('E') {
            return Type::Float;
        }
    }

    // Check for float with thousand separators
    if FLOAT_THOUSANDS_PATTERN.is_match(trimmed) {
        return Type::Float;
    }

    // Check for ISO datetime first (more specific)
    if DATETIME_ISO_PATTERN.is_match(trimmed) || DATETIME_GENERAL_PATTERN.is_match(trimmed) {
        return Type::DateTime;
    }

    // Check for dates
    if DATE_ISO_PATTERN.is_match(trimmed)
        || DATE_US_PATTERN.is_match(trimmed)
        || DATE_EURO_PATTERN.is_match(trimmed)
    {
        return Type::Date;
    }

    // Fallback to text
    Type::Text
}

/// Calculate the type score for a table.
///
/// This score measures how well the values in each column conform to
/// consistent data types. Higher scores indicate better type consistency.
///
/// Optimized: Single pass through all cells, tracking type counts for all columns simultaneously.
pub fn calculate_type_score(table: &Table) -> f64 {
    if table.is_empty() {
        return 0.0;
    }

    let num_cols = table.modal_field_count();
    if num_cols == 0 {
        return 0.0;
    }

    // Track type counts for ALL columns in one pass
    // Each column gets an array of type counts
    let mut col_type_counts: Vec<[usize; Type::COUNT]> = vec![[0; Type::COUNT]; num_cols];
    let mut col_totals: Vec<usize> = vec![0; num_cols];

    // Single pass through all rows and cells
    for row in &table.rows {
        for (col_idx, cell) in row.iter().enumerate().take(num_cols) {
            let cell_type = detect_cell_type(cell);
            col_type_counts[col_idx][cell_type.as_index()] += 1;
            col_totals[col_idx] += 1;
        }
    }

    // Calculate scores from accumulated counts
    let mut total_score = 0.0;
    let mut valid_cols = 0;

    for col_idx in 0..num_cols {
        let score = compute_consistency_from_counts(&col_type_counts[col_idx], col_totals[col_idx]);
        if score > 0.0 {
            total_score += score;
            valid_cols += 1;
        }
    }

    if valid_cols == 0 {
        return 0.0;
    }

    total_score / valid_cols as f64
}

/// Compute type consistency score from pre-computed type counts.
#[inline]
fn compute_consistency_from_counts(type_counts: &[usize; Type::COUNT], total_cells: usize) -> f64 {
    if total_cells == 0 {
        return 0.0;
    }

    // Special handling: NULL values shouldn't penalize the score
    let null_count = type_counts[Type::NULL.as_index()];
    let non_null_total = total_cells - null_count;

    if non_null_total == 0 {
        // All nulls - neutral score
        return 0.5;
    }

    // Calculate consistency excluding nulls
    let max_non_null = type_counts
        .iter()
        .enumerate()
        .filter(|&(i, _)| i != Type::NULL.as_index())
        .map(|(_, &c)| c)
        .max()
        .unwrap_or(0);

    max_non_null as f64 / non_null_total as f64
}

/// Infer the type for each column in a table.
pub fn infer_column_types(table: &Table) -> Vec<Type> {
    let num_cols = table.modal_field_count();
    let mut types = Vec::with_capacity(num_cols);

    for col_idx in 0..num_cols {
        types.push(infer_single_column_type(table, col_idx));
    }

    types
}

/// Infer the type for a single column.
fn infer_single_column_type(table: &Table, col_idx: usize) -> Type {
    let mut merged_type = Type::NULL;

    for row in &table.rows {
        if col_idx < row.len() {
            let cell_type = detect_cell_type(&row[col_idx]);
            merged_type = merged_type.merge(cell_type);
        }
    }

    merged_type
}

/// Calculate the pattern score for a value.
///
/// This gives a weighted score based on how specific the detected pattern is.
/// More specific patterns (like datetime) score higher than generic ones (like text).
pub fn pattern_specificity_score(value: &str) -> f64 {
    let trimmed = value.trim();

    if trimmed.is_empty() {
        return 0.0;
    }

    // Check patterns in order of specificity (uses cached static slice)
    for pc in get_pattern_categories() {
        if pc.pattern.is_match(trimmed) {
            return pc.weight;
        }
    }

    // Text is the fallback with lowest specificity
    0.1
}

/// Calculate the average pattern specificity score for a table.
pub fn calculate_pattern_score(table: &Table) -> f64 {
    if table.is_empty() {
        return 0.0;
    }

    let mut total_score = 0.0;
    let mut count = 0;

    for row in &table.rows {
        for cell in row {
            total_score += pattern_specificity_score(cell);
            count += 1;
        }
    }

    if count == 0 {
        return 0.0;
    }

    total_score / count as f64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_cell_type() {
        assert_eq!(detect_cell_type("123"), Type::Unsigned);
        assert_eq!(detect_cell_type("-123"), Type::Signed);
        assert_eq!(detect_cell_type("12.34"), Type::Float);
        assert_eq!(detect_cell_type("true"), Type::Boolean);
        assert_eq!(detect_cell_type("2023-12-31"), Type::Date);
        assert_eq!(detect_cell_type("2023-12-31T12:30:45"), Type::DateTime);
        assert_eq!(detect_cell_type("hello"), Type::Text);
        assert_eq!(detect_cell_type(""), Type::NULL);
        assert_eq!(detect_cell_type("NULL"), Type::NULL);
    }

    #[test]
    fn test_infer_column_types() {
        let mut table = Table::new();
        table.rows = vec![
            vec![
                "1".to_string(),
                "hello".to_string(),
                "2023-01-01".to_string(),
            ],
            vec![
                "2".to_string(),
                "world".to_string(),
                "2023-01-02".to_string(),
            ],
            vec![
                "3".to_string(),
                "test".to_string(),
                "2023-01-03".to_string(),
            ],
        ];
        table.field_counts = vec![3, 3, 3];
        table.update_modal_field_count();

        let types = infer_column_types(&table);
        assert_eq!(types, vec![Type::Unsigned, Type::Text, Type::Date]);
    }
}

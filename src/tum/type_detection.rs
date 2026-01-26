//! Type detection for CSV cells using regex patterns.

use super::regexes::*;
use super::table::Table;
use crate::field_type::Type;
use foldhash::{HashMap, HashMapExt};

/// Detect the type of a single cell value.
pub fn detect_cell_type(value: &str) -> Type {
    let trimmed = value.trim();

    // Check for empty first
    if trimmed.is_empty() || EMPTY_PATTERN.is_match(trimmed) {
        return Type::NULL;
    }

    // Check for NULL-like values
    if NULL_PATTERN.is_match(trimmed) {
        return Type::NULL;
    }

    // Check for unsigned integer (must come before boolean since 1/0 match boolean)
    if UNSIGNED_PATTERN.is_match(trimmed) && !trimmed.starts_with('-') {
        return Type::Unsigned;
    }

    // Check for signed integer
    if SIGNED_PATTERN.is_match(trimmed) {
        return Type::Signed;
    }

    // Check for boolean (after integers so "1" and "0" are treated as numbers)
    if BOOLEAN_PATTERN.is_match(trimmed) {
        return Type::Boolean;
    }

    // Check for float
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
pub fn calculate_type_score(table: &Table) -> f64 {
    if table.is_empty() {
        return 0.0;
    }

    let num_cols = table.modal_field_count();
    if num_cols == 0 {
        return 0.0;
    }

    let mut total_score = 0.0;
    let mut valid_cols = 0;

    for col_idx in 0..num_cols {
        let col_score = column_type_consistency(table, col_idx);
        if col_score > 0.0 {
            total_score += col_score;
            valid_cols += 1;
        }
    }

    if valid_cols == 0 {
        return 0.0;
    }

    total_score / valid_cols as f64
}

/// Calculate type consistency for a single column.
fn column_type_consistency(table: &Table, col_idx: usize) -> f64 {
    let mut type_counts: HashMap<Type, usize> = HashMap::with_capacity(8);
    let mut total_cells = 0;

    for row in &table.rows {
        if col_idx < row.len() {
            let cell_type = detect_cell_type(&row[col_idx]);
            *type_counts.entry(cell_type).or_insert(0) += 1;
            total_cells += 1;
        }
    }

    if total_cells == 0 {
        return 0.0;
    }

    // Calculate the fraction of cells that match the dominant type
    let _max_count = type_counts.values().copied().max().unwrap_or(0);

    // Special handling: NULL values shouldn't penalize the score
    let null_count = type_counts.get(&Type::NULL).copied().unwrap_or(0);
    let non_null_total = total_cells - null_count;

    if non_null_total == 0 {
        // All nulls - neutral score
        return 0.5;
    }

    // Calculate consistency excluding nulls
    let max_non_null = type_counts
        .iter()
        .filter(|&(&t, _)| t != Type::NULL)
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

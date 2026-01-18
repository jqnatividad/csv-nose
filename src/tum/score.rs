//! Combined scoring for dialect detection.
//!
//! The gamma score combines uniformity and type detection scores
//! to rank potential CSV dialects.

use super::potential_dialects::PotentialDialect;
use super::table::{parse_table, Table};
use super::type_detection::{calculate_pattern_score, calculate_type_score};
use super::uniformity::{calculate_tau_0, calculate_tau_1, is_uniform};

/// Score result for a dialect.
#[derive(Debug, Clone)]
pub struct DialectScore {
    /// The potential dialect that was scored.
    pub dialect: PotentialDialect,
    /// The combined gamma score (higher is better).
    pub gamma: f64,
    /// Consistency score (tau_0).
    #[allow(dead_code)]
    pub tau_0: f64,
    /// Dispersion score (tau_1).
    #[allow(dead_code)]
    pub tau_1: f64,
    /// Type detection score.
    #[allow(dead_code)]
    pub type_score: f64,
    /// Pattern specificity score.
    #[allow(dead_code)]
    pub pattern_score: f64,
    /// Number of rows parsed.
    #[allow(dead_code)]
    pub num_rows: usize,
    /// Modal (most common) field count.
    pub num_fields: usize,
    /// Whether the table has uniform field counts.
    pub is_uniform: bool,
}

impl DialectScore {
    /// Create a new score result.
    pub fn new(dialect: PotentialDialect, table: &Table) -> Self {
        let tau_0 = calculate_tau_0(table);
        let tau_1 = calculate_tau_1(table);
        let type_score = calculate_type_score(table);
        let pattern_score = calculate_pattern_score(table);
        let uniform = is_uniform(table);

        // Calculate combined gamma score
        let gamma = compute_gamma(tau_0, tau_1, type_score, pattern_score, table);

        Self {
            dialect,
            gamma,
            tau_0,
            tau_1,
            type_score,
            pattern_score,
            num_rows: table.num_rows(),
            num_fields: table.modal_field_count(),
            is_uniform: uniform,
        }
    }

    /// Create a zero score (for failed parses).
    pub fn zero(dialect: PotentialDialect) -> Self {
        Self {
            dialect,
            gamma: 0.0,
            tau_0: 0.0,
            tau_1: 0.0,
            type_score: 0.0,
            pattern_score: 0.0,
            num_rows: 0,
            num_fields: 0,
            is_uniform: false,
        }
    }
}

/// Compute the combined gamma score.
///
/// The gamma score combines multiple factors:
/// - tau_0 (consistency): higher is better
/// - tau_1 (dispersion): higher is better (less dispersion)
/// - type_score: higher means better type consistency
/// - pattern_score: higher means more specific patterns detected
/// - Additional bonuses for uniform tables and reasonable field counts
fn compute_gamma(
    tau_0: f64,
    tau_1: f64,
    type_score: f64,
    pattern_score: f64,
    table: &Table,
) -> f64 {
    if table.is_empty() {
        return 0.0;
    }

    // Base score from uniformity metrics
    let uniformity_score = (tau_0 * tau_1).sqrt();

    // Type detection contributes to the score
    let type_contribution = type_score * 0.3;

    // Pattern specificity provides additional signal
    let pattern_contribution = pattern_score * 0.1;

    // Bonus for having multiple rows (more data is more reliable)
    let row_bonus = (table.num_rows().min(20) as f64 / 20.0) * 0.1;

    // Bonus for having multiple fields (single field might be wrong delimiter)
    let field_count = table.modal_field_count();
    let field_bonus = if field_count >= 2 {
        (field_count.min(10) as f64 / 10.0) * 0.2
    } else {
        0.0
    };

    // Penalty for single-field tables (likely wrong delimiter)
    let single_field_penalty = if field_count == 1 { 0.5 } else { 1.0 };

    // Penalty for extremely high field counts (might be splitting on wrong char)
    let high_field_penalty = if field_count > 100 {
        0.5
    } else if field_count > 50 {
        0.8
    } else {
        1.0
    };

    // Combine all factors
    let raw_score =
        uniformity_score * 0.5 + type_contribution + pattern_contribution + row_bonus + field_bonus;

    raw_score * single_field_penalty * high_field_penalty
}

/// Score a dialect against the data.
///
/// Returns the DialectScore which includes the gamma score and component scores.
pub fn score_dialect(data: &[u8], dialect: &PotentialDialect, max_rows: usize) -> DialectScore {
    let table = parse_table(data, dialect, max_rows);

    if table.is_empty() {
        return DialectScore::zero(dialect.clone());
    }

    DialectScore::new(dialect.clone(), &table)
}

/// Find the best scoring dialect from a list.
pub fn find_best_dialect(scores: &[DialectScore]) -> Option<&DialectScore> {
    scores.iter().filter(|s| s.gamma > 0.0).max_by(|a, b| {
        a.gamma
            .partial_cmp(&b.gamma)
            .unwrap_or(std::cmp::Ordering::Equal)
    })
}

/// Score all potential dialects and return sorted by gamma score (descending).
pub fn score_all_dialects(
    data: &[u8],
    dialects: &[PotentialDialect],
    max_rows: usize,
) -> Vec<DialectScore> {
    let mut scores: Vec<DialectScore> = dialects
        .iter()
        .map(|d| score_dialect(data, d, max_rows))
        .collect();

    // Sort by gamma score descending
    scores.sort_by(|a, b| {
        b.gamma
            .partial_cmp(&a.gamma)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    scores
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::metadata::Quote;
    use crate::tum::potential_dialects::LineTerminator;

    #[test]
    fn test_score_simple_csv() {
        let data = b"a,b,c\n1,2,3\n4,5,6\n";
        let dialect = PotentialDialect::new(b',', Quote::Some(b'"'), LineTerminator::LF);

        let score = score_dialect(data, &dialect, 100);
        assert!(score.gamma > 0.0);
        assert_eq!(score.num_fields, 3);
        assert!(score.is_uniform);
    }

    #[test]
    fn test_wrong_delimiter_lower_score() {
        let data = b"a,b,c\n1,2,3\n4,5,6\n";

        let correct_dialect = PotentialDialect::new(b',', Quote::Some(b'"'), LineTerminator::LF);
        let wrong_dialect = PotentialDialect::new(b';', Quote::Some(b'"'), LineTerminator::LF);

        let correct_score = score_dialect(data, &correct_dialect, 100);
        let wrong_score = score_dialect(data, &wrong_dialect, 100);

        assert!(correct_score.gamma > wrong_score.gamma);
    }

    #[test]
    fn test_find_best_dialect() {
        let data = b"a,b,c\n1,2,3\n4,5,6\n";
        let dialects = vec![
            PotentialDialect::new(b',', Quote::Some(b'"'), LineTerminator::LF),
            PotentialDialect::new(b';', Quote::Some(b'"'), LineTerminator::LF),
            PotentialDialect::new(b'\t', Quote::Some(b'"'), LineTerminator::LF),
        ];

        let scores = score_all_dialects(data, &dialects, 100);
        let best = find_best_dialect(&scores).unwrap();

        assert_eq!(best.dialect.delimiter, b',');
    }
}

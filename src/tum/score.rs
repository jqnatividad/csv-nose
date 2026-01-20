//! Combined scoring for dialect detection.
//!
//! The gamma score combines uniformity and type detection scores
//! to rank potential CSV dialects.

use super::potential_dialects::PotentialDialect;
use super::table::{Table, parse_table};
use super::type_detection::{calculate_pattern_score, calculate_type_score};
use super::uniformity::{calculate_tau_0, calculate_tau_1, is_uniform};

/// Pre-computed quote character counts for the data.
/// Used to avoid redundant byte counting across multiple dialect evaluations.
#[derive(Debug, Clone, Copy)]
struct QuoteCounts {
    double: usize,
    single: usize,
    data_len: usize,
}

impl QuoteCounts {
    fn new(data: &[u8]) -> Self {
        Self {
            double: bytecount::count(data, b'"'),
            single: bytecount::count(data, b'\''),
            data_len: data.len(),
        }
    }
}

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

        // Calculate combined gamma score (includes delimiter penalty)
        let gamma = compute_gamma(
            tau_0,
            tau_1,
            type_score,
            pattern_score,
            table,
            dialect.delimiter,
        );

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
    pub const fn zero(dialect: PotentialDialect) -> Self {
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
/// - Penalties for uncommon delimiters
fn compute_gamma(
    tau_0: f64,
    tau_1: f64,
    type_score: f64,
    pattern_score: f64,
    table: &Table,
    delimiter: u8,
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

    // Penalty for very small samples (less reliable detection)
    let num_rows = table.num_rows();
    let small_sample_penalty = if num_rows < 3 {
        0.70 // Very small - high unreliability
    } else if num_rows < 5 {
        0.85 // Small - moderate unreliability
    } else {
        1.0
    };

    // Penalty for uncommon delimiters
    // This helps prevent rare characters from winning due to accidental patterns
    let delimiter_penalty = match delimiter {
        b',' | b';' | b'\t' => 1.0, // Common delimiters - no penalty
        b'|' => 0.98,               // Pipe - slight penalty
        b':' => 0.90,               // Colon - moderate penalty (often in timestamps)
        b' ' => 0.75,               // Space - significant penalty (often in text)
        b'^' | b'~' => 0.80,        // Rare delimiters
        b'#' => 0.60,               // Hash - often comment marker
        b'&' => 0.60,               // Ampersand - very rare
        0xA7 => 0.70,               // Section sign (§) - rare delimiter
        b'/' => 0.65,               // Forward slash - rare, often in paths/dates
        _ => 0.70,                  // Unknown - penalty
    };

    // Combine all factors
    // uniformity_score * 0.5 + type_contribution + pattern_contribution + row_bonus + field_bonus;
    let raw_score = uniformity_score.mul_add(0.5, type_contribution)
        + pattern_contribution
        + row_bonus
        + field_bonus;

    raw_score * single_field_penalty * high_field_penalty * delimiter_penalty * small_sample_penalty
}

/// Score a dialect against the data.
///
/// Returns the DialectScore which includes the gamma score and component scores.
#[allow(dead_code)]
pub fn score_dialect(data: &[u8], dialect: &PotentialDialect, max_rows: usize) -> DialectScore {
    let quote_counts = QuoteCounts::new(data);
    let (score, _table) = score_dialect_with_counts(data, dialect, max_rows, &quote_counts);
    score
}

/// Score a dialect against the data with pre-computed quote counts.
///
/// This is the internal implementation that accepts pre-computed QuoteCounts
/// to avoid redundant byte counting when scoring multiple dialects.
/// Returns both the score and the parsed table for potential reuse.
fn score_dialect_with_counts(
    data: &[u8],
    dialect: &PotentialDialect,
    max_rows: usize,
    quote_counts: &QuoteCounts,
) -> (DialectScore, Table) {
    let table = parse_table(data, dialect, max_rows);

    if table.is_empty() {
        return (DialectScore::zero(dialect.clone()), table);
    }

    let mut score = DialectScore::new(dialect.clone(), &table);

    // Apply quote evidence scoring using pre-computed counts
    let quote_multiplier = quote_evidence_score_with_counts(quote_counts, dialect);
    score.gamma *= quote_multiplier;

    (score, table)
}

/// Calculate a score multiplier based on quote character evidence in the data.
///
/// This function examines the actual presence of quote characters in the data
/// to boost dialects where the quote char is genuinely used and penalize
/// Quote::None when quotes are present.
///
/// The scoring is conservative to avoid false positives from apostrophes
/// in text content (e.g., "John's" contains a single quote but isn't quoted).
#[allow(dead_code)]
fn quote_evidence_score(data: &[u8], dialect: &PotentialDialect) -> f64 {
    let quote_counts = QuoteCounts::new(data);
    quote_evidence_score_with_counts(&quote_counts, dialect)
}

/// Calculate quote evidence score using pre-computed quote counts.
/// This avoids redundant byte counting when scoring multiple dialects.
fn quote_evidence_score_with_counts(quote_counts: &QuoteCounts, dialect: &PotentialDialect) -> f64 {
    use crate::metadata::Quote;

    if quote_counts.data_len == 0 {
        return 1.0;
    }

    // Calculate density (quotes per 1000 bytes) - higher density suggests quoting
    let double_density = (quote_counts.double * 1000) / quote_counts.data_len;
    let single_density = (quote_counts.single * 1000) / quote_counts.data_len;

    // Threshold: need at least ~0.5% quote density to consider it significant
    // This filters out incidental apostrophes in text
    let min_density_threshold = 5; // 0.5% = 5 per 1000

    match dialect.quote {
        Quote::Some(b'"') => {
            if double_density >= min_density_threshold {
                // Double quotes have significant density - slight boost
                1.03
            } else {
                // Neutral - rely on other scoring factors
                1.0
            }
        }
        Quote::Some(b'\'') => {
            // Single quotes are tricky because apostrophes are common in text
            // Only boost if single quotes dominate AND double quotes are absent
            if single_density >= min_density_threshold * 2 && double_density < min_density_threshold
            {
                // Strong evidence of single-quote usage
                1.05
            } else if double_density >= min_density_threshold {
                // Double quotes present but testing single - penalty
                0.95
            } else {
                1.0
            }
        }
        Quote::None => {
            // Only penalize Quote::None when there's strong quoting evidence
            if double_density >= min_density_threshold {
                0.90
            } else {
                1.0
            }
        }
        Quote::Some(_) => 1.0, // Other quote chars - neutral
    }
}

/// Find the best scoring dialect from a list.
///
/// When dialects have similar scores, this function prefers:
/// 1. Common delimiters (comma, semicolon, tab) over rare ones (space, #, &)
/// 2. Dialects with Quote::Some(b'"') over Quote::None (standard default)
/// 3. Dialects with Quote::Some(b'"') over Quote::Some(b'\'')
pub fn find_best_dialect(scores: &[DialectScore]) -> Option<&DialectScore> {
    // First, check if all dialects result in single-field tables
    // In that case, prefer comma as the default delimiter
    let all_single_field = scores
        .iter()
        .filter(|s| s.gamma > 0.0)
        .all(|s| s.num_fields <= 1);

    scores.iter().filter(|s| s.gamma > 0.0).max_by(|a, b| {
        // If scores are very close (within 10%), use delimiter and quote preference
        let score_ratio = if a.gamma > b.gamma {
            b.gamma / a.gamma
        } else {
            a.gamma / b.gamma
        };

        // For single-field tables, prefer comma delimiter and double-quote
        if all_single_field {
            let a_delim_priority = delimiter_priority(a.dialect.delimiter);
            let b_delim_priority = delimiter_priority(b.dialect.delimiter);

            match a_delim_priority.cmp(&b_delim_priority) {
                std::cmp::Ordering::Equal => {
                    // Same delimiter priority, use quote preference
                    let a_quote_priority = quote_priority(a.dialect.quote);
                    let b_quote_priority = quote_priority(b.dialect.quote);
                    return a_quote_priority.cmp(&b_quote_priority);
                }
                other => return other,
            }
        }

        if score_ratio > 0.90 {
            // Scores are close, use delimiter priority first, then quote priority
            let a_delim_priority = delimiter_priority(a.dialect.delimiter);
            let b_delim_priority = delimiter_priority(b.dialect.delimiter);

            match a_delim_priority.cmp(&b_delim_priority) {
                std::cmp::Ordering::Equal => {
                    // Delimiters have same priority, check quotes
                    let a_quote_priority = quote_priority(a.dialect.quote);
                    let b_quote_priority = quote_priority(b.dialect.quote);

                    match a_quote_priority.cmp(&b_quote_priority) {
                        std::cmp::Ordering::Equal => a
                            .gamma
                            .partial_cmp(&b.gamma)
                            .unwrap_or(std::cmp::Ordering::Equal),
                        other => other,
                    }
                }
                other => other,
            }
        } else {
            // Scores are different enough, use gamma directly
            a.gamma
                .partial_cmp(&b.gamma)
                .unwrap_or(std::cmp::Ordering::Equal)
        }
    })
}

/// Returns a priority score for delimiters (higher = preferred).
/// Common delimiters like comma are preferred over rare ones like space or &.
const fn delimiter_priority(delimiter: u8) -> u8 {
    match delimiter {
        b',' => 10, // Comma - most common, highest priority
        b';' => 9,  // Semicolon - common in European locales
        b'\t' => 8, // Tab - TSV files
        b'|' => 7,  // Pipe - common in data exports
        b':' => 4,  // Colon - sometimes used, but also appears in timestamps
        b'^' => 3,  // Caret - rare
        b'~' => 3,  // Tilde - rare
        0xA7 => 2,  // Section sign (§) - rare
        b'/' => 2,  // Forward slash - rare
        b' ' => 2,  // Space - very rare as delimiter, often appears in text
        b'#' => 1,  // Hash - very rare, often used for comments
        b'&' => 1,  // Ampersand - very rare
        _ => 0,     // Unknown delimiters - lowest priority
    }
}

/// Returns a priority score for quote characters (higher = preferred).
/// Double-quote is the standard default and should be preferred.
const fn quote_priority(quote: crate::metadata::Quote) -> u8 {
    use crate::metadata::Quote;
    match quote {
        Quote::Some(b'"') => 3,  // Standard default - highest priority
        Quote::Some(b'\'') => 2, // Single quote - second priority
        Quote::None => 1,        // No quoting - lowest priority
        Quote::Some(_) => 0,     // Other quote chars - very low priority
    }
}

/// Score all potential dialects and return sorted by gamma score (descending).
#[allow(dead_code)]
pub fn score_all_dialects(
    data: &[u8],
    dialects: &[PotentialDialect],
    max_rows: usize,
) -> Vec<DialectScore> {
    let (scores, _) = score_all_dialects_with_best_table(data, dialects, max_rows);
    scores
}

/// Score all potential dialects and return sorted by gamma score (descending),
/// along with the parsed table of the best-scoring dialect.
///
/// This avoids re-parsing the best dialect's data for preamble detection
/// and metadata building.
pub fn score_all_dialects_with_best_table(
    data: &[u8],
    dialects: &[PotentialDialect],
    max_rows: usize,
) -> (Vec<DialectScore>, Option<Table>) {
    // Pre-compute quote counts once for all dialect evaluations
    let quote_counts = QuoteCounts::new(data);

    // Score all dialects and keep track of the best table
    let mut best_table: Option<Table> = None;
    let mut best_gamma: f64 = f64::NEG_INFINITY;

    let mut scores: Vec<DialectScore> = dialects
        .iter()
        .map(|d| {
            let (score, table) = score_dialect_with_counts(data, d, max_rows, &quote_counts);
            // Track the table with the highest gamma score
            if score.gamma > best_gamma {
                best_gamma = score.gamma;
                best_table = Some(table);
            }
            score
        })
        .collect();

    // Sort by gamma score descending
    scores.sort_by(|a, b| {
        b.gamma
            .partial_cmp(&a.gamma)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    (scores, best_table)
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

//! Combined scoring for dialect detection.
//!
//! The gamma score combines uniformity and type detection scores
//! to rank potential CSV dialects.

use std::cell::RefCell;

use rayon::prelude::*;

use super::potential_dialects::PotentialDialect;
use super::table::{Table, parse_table, parse_table_normalized};
use super::type_detection::{TypeScoreBuffers, calculate_pattern_score, calculate_type_score};
use super::uniformity::{calculate_tau_0, calculate_tau_1, is_uniform};

thread_local! {
    // Each rayon worker thread owns one reusable TypeScoreBuffers.  Vec::clear()
    // keeps the allocated capacity, so after sniffing a very-wide CSV a thread's
    // buffer retains the high-water-mark allocation for the lifetime of the rayon
    // pool (typically the whole process).  Overhead is small
    // (max_cols × Type::COUNT × 2 × sizeof(usize) per thread) but worth noting
    // for long-running library users processing a mix of narrow and wide files.
    static BUFFERS: RefCell<TypeScoreBuffers> = RefCell::new(TypeScoreBuffers::new());
}

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

/// Pre-computed quote boundary counts for both quote characters.
/// Used to avoid redundant data scanning across multiple dialect evaluations.
#[derive(Debug, Clone)]
struct QuoteBoundaryCounts {
    /// Boundary counts for double quote with each delimiter (opening + closing)
    double_boundaries: Vec<(u8, usize)>,
    /// Boundary counts for single quote with each delimiter (opening + closing)
    single_boundaries: Vec<(u8, usize)>,
    /// Opening-only boundary counts for single quote with each delimiter
    /// (delimiter/newline → quote, field start).  Used to distinguish genuine
    /// quoting from apostrophes that appear only before delimiters (closing).
    single_opening_boundaries: Vec<(u8, usize)>,
    /// Newline boundary counts for double quote (not delimiter-specific)
    double_newline_boundaries: usize,
    /// Newline boundary counts for single quote (not delimiter-specific)
    single_newline_boundaries: usize,
    /// Opening-only newline boundary counts for single quote
    single_opening_newline_boundaries: usize,
    /// Whether data starts with double quote
    starts_with_double: bool,
    /// Whether data starts with single quote
    starts_with_single: bool,
}

impl QuoteBoundaryCounts {
    /// Compute quote boundary counts for all delimiters in a single pass.
    fn new(data: &[u8], delimiters: &[u8]) -> Self {
        let mut double_counts: Vec<usize> = vec![0; delimiters.len()];
        let mut single_counts: Vec<usize> = vec![0; delimiters.len()];
        let mut single_opening_counts: Vec<usize> = vec![0; delimiters.len()];
        let mut double_newline_boundaries: usize = 0;
        let mut single_newline_boundaries: usize = 0;
        let mut single_opening_newline_boundaries: usize = 0;

        // Create lookup table for delimiter indices
        let mut delim_indices = [usize::MAX; 256];
        for (i, &d) in delimiters.iter().enumerate() {
            delim_indices[d as usize] = i;
        }

        // Single pass through data for all delimiters
        for window in data.windows(2) {
            let is_newline = window[0] == b'\n' || window[0] == b'\r';
            let delim_idx = delim_indices[window[0] as usize];
            let is_delimiter = delim_idx != usize::MAX;

            // Quote after delimiter/newline (field start = OPENING boundary)
            if is_newline || is_delimiter {
                if window[1] == b'"' {
                    if is_newline {
                        // Count newline boundaries separately (once, not per delimiter)
                        double_newline_boundaries += 1;
                    } else {
                        // Count delimiter-specific boundary
                        double_counts[delim_idx] += 1;
                    }
                }
                if window[1] == b'\'' {
                    if is_newline {
                        single_newline_boundaries += 1;
                        single_opening_newline_boundaries += 1;
                    } else {
                        single_counts[delim_idx] += 1;
                        single_opening_counts[delim_idx] += 1;
                    }
                }
            }

            // Quote before delimiter/newline (field end = CLOSING boundary)
            let is_end_newline = window[1] == b'\n' || window[1] == b'\r';
            let end_delim_idx = delim_indices[window[1] as usize];
            let is_end_delimiter = end_delim_idx != usize::MAX;

            if window[0] == b'"' && (is_end_newline || is_end_delimiter) {
                if is_end_newline {
                    double_newline_boundaries += 1;
                } else {
                    double_counts[end_delim_idx] += 1;
                }
            }
            if window[0] == b'\'' && (is_end_newline || is_end_delimiter) {
                if is_end_newline {
                    single_newline_boundaries += 1;
                } else {
                    single_counts[end_delim_idx] += 1;
                }
            }
        }

        let starts_with_double = !data.is_empty() && data[0] == b'"';
        let starts_with_single = !data.is_empty() && data[0] == b'\'';

        Self {
            double_boundaries: delimiters.iter().copied().zip(double_counts).collect(),
            single_boundaries: delimiters.iter().copied().zip(single_counts).collect(),
            single_opening_boundaries: delimiters
                .iter()
                .copied()
                .zip(single_opening_counts)
                .collect(),
            double_newline_boundaries,
            single_newline_boundaries,
            single_opening_newline_boundaries,
            starts_with_double,
            starts_with_single,
        }
    }

    /// Get the boundary count for a specific quote character and delimiter.
    fn get_boundary_count(&self, quote_char: u8, delimiter: u8) -> usize {
        let (boundaries, newline_boundaries) = if quote_char == b'"' {
            (&self.double_boundaries, self.double_newline_boundaries)
        } else {
            (&self.single_boundaries, self.single_newline_boundaries)
        };

        let delimiter_count = boundaries
            .iter()
            .find(|&&(d, _)| d == delimiter)
            .map_or(0, |&(_, c)| c);

        // Add 1 if data starts with this quote char
        let starts_with_quote = (quote_char == b'"' && self.starts_with_double)
            || (quote_char == b'\'' && self.starts_with_single);
        let start_bonus = usize::from(starts_with_quote);

        // Combine delimiter-specific count with newline boundaries (which apply to all delimiters)
        delimiter_count + newline_boundaries + start_bonus
    }

    /// Get the opening-only boundary count for single-quote with a given delimiter.
    ///
    /// Opening boundaries are delimiter/newline → single-quote transitions (field starts).
    /// This distinguishes genuine single-quote quoting (both opening and closing boundaries)
    /// from apostrophes that appear only before delimiters (closing only, as in `'value',`).
    fn get_single_opening_boundary_count(&self, delimiter: u8) -> usize {
        let delimiter_count = self
            .single_opening_boundaries
            .iter()
            .find(|&&(d, _)| d == delimiter)
            .map_or(0, |&(_, c)| c);

        // starts_with_single is an opening boundary (file-start → single-quote)
        let start_bonus = usize::from(self.starts_with_single);

        delimiter_count + self.single_opening_newline_boundaries + start_bonus
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
    pub fn new(dialect: PotentialDialect, table: &Table, type_score: f64) -> Self {
        let tau_0 = calculate_tau_0(table);
        let tau_1 = calculate_tau_1(table);
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
        0.80 // Very small - high unreliability
    } else if num_rows < 5 {
        0.90 // Small - moderate unreliability
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
        0xA7 => 0.78,               // Section sign (§) - rare but legitimate delimiter
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
    let mut buffers = TypeScoreBuffers::new();
    let (score, _table) =
        score_dialect_with_counts(data, dialect, max_rows, &quote_counts, &mut buffers);
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
    buffers: &mut TypeScoreBuffers,
) -> (DialectScore, Table) {
    let table = parse_table(data, dialect, max_rows);

    if table.is_empty() {
        return (DialectScore::zero(dialect.clone()), table);
    }

    let type_score = calculate_type_score(&table, buffers);
    let mut score = DialectScore::new(dialect.clone(), &table, type_score);

    // Apply quote evidence scoring using pre-computed counts and raw data for boundary detection
    let quote_multiplier = quote_evidence_score_with_data(data, quote_counts, dialect);
    score.gamma *= quote_multiplier;

    (score, table)
}

/// Score a dialect against pre-normalized data with pre-computed quote counts.
///
/// This variant assumes the data has already been normalized to LF line endings
/// for better performance when scoring multiple dialects.
fn score_dialect_with_normalized_data(
    normalized_data: &[u8],
    dialect: &PotentialDialect,
    max_rows: usize,
    quote_counts: &QuoteCounts,
    boundary_counts: &QuoteBoundaryCounts,
    buffers: &mut TypeScoreBuffers,
) -> (DialectScore, Table) {
    let table = parse_table_normalized(normalized_data, dialect, max_rows);

    if table.is_empty() {
        return (DialectScore::zero(dialect.clone()), table);
    }

    let type_score = calculate_type_score(&table, buffers);
    let mut score = DialectScore::new(dialect.clone(), &table, type_score);

    // Apply quote evidence scoring using pre-computed counts and cached boundary counts
    let quote_multiplier =
        quote_evidence_score_with_cached_boundaries(quote_counts, boundary_counts, dialect);

    // Dampen the quote boost when the first row has just 1 field AND the non-modal rows
    // exhibit diverse field counts (≥3 distinct values). This prevents JSON-content-in-
    // unquoted-fields from triggering a false 2.2x boost: e.g. a tab-delimited file where
    // unquoted JSON fields contain `,key"` patterns that look like opening quote boundaries
    // for comma+doublequote. In such files the first row (tab-delimited header) has 0 commas
    // → 1 field, and JSON data rows have wildly varying comma counts (e.g., 1, 46, 32, 19).
    //
    // The distinguishing check: if the rows that deviate from the modal all share the same
    // count (like {1, 1, 1} for preamble title rows), the non-uniformity is just preamble.
    // If the non-modal rows have ≥3 distinct field counts, the whole table is chaotically
    // variable — a strong signal that boundaries come from field content, not real quoting.
    let effective_multiplier =
        if quote_multiplier > 1.5 && score.num_fields >= 5 && !score.is_uniform {
            let first_fields = table
                .field_counts
                .first()
                .copied()
                .unwrap_or(score.num_fields);
            if first_fields <= 1 {
                // Count distinct field counts among non-modal rows.
                let modal = score.num_fields;
                let mut distinct_counts: Vec<usize> = table
                    .field_counts
                    .iter()
                    .filter(|&&c| c != modal)
                    .copied()
                    .collect();
                distinct_counts.sort_unstable();
                distinct_counts.dedup();
                let distinct_non_modal = distinct_counts.len();
                if distinct_non_modal >= 3 {
                    // ≥3 distinct non-modal field counts → genuinely chaotic table, not just
                    // a small preamble. Scale boost down to 30% of excess so the correct
                    // dialect can compete.
                    1.0 + (quote_multiplier - 1.0) * 0.3
                } else {
                    quote_multiplier
                }
            } else {
                quote_multiplier
            }
        } else {
            quote_multiplier
        };
    score.gamma *= effective_multiplier;

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
                // Double quotes have significant density - boost
                1.06
            } else {
                // Neutral - rely on other scoring factors
                1.0
            }
        }
        Quote::Some(b'\'') => {
            // Single quotes are tricky because apostrophes are common in text
            // Only boost if single quotes dominate AND double quotes are absent
            if double_density == 0 && single_density >= min_density_threshold {
                // No double quotes at all - strong single-quote evidence
                1.10
            } else if single_density >= min_density_threshold * 2
                && double_density < min_density_threshold
            {
                // Strong evidence of single-quote usage
                1.05
            } else if double_density >= min_density_threshold {
                // Double quotes present but testing single - stronger penalty
                0.92
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

/// Check if quote characters appear at field boundaries (stronger evidence).
/// Returns the count of boundary pairs found.
#[allow(dead_code)]
fn quote_boundary_count(data: &[u8], quote_char: u8, delimiter: u8) -> usize {
    let mut boundary_pairs = 0;
    for window in data.windows(2) {
        // Quote after delimiter/newline (field start)
        if (window[0] == delimiter || window[0] == b'\n' || window[0] == b'\r')
            && window[1] == quote_char
        {
            boundary_pairs += 1;
        }
        // Quote before delimiter/newline (field end)
        if window[0] == quote_char
            && (window[1] == delimiter || window[1] == b'\n' || window[1] == b'\r')
        {
            boundary_pairs += 1;
        }
    }
    // Also check start of data
    if !data.is_empty() && data[0] == quote_char {
        boundary_pairs += 1;
    }
    boundary_pairs
}

/// Calculate quote evidence score using pre-computed counts and cached boundary counts.
/// This is the optimized version that avoids redundant data scanning.
fn quote_evidence_score_with_cached_boundaries(
    quote_counts: &QuoteCounts,
    boundary_counts: &QuoteBoundaryCounts,
    dialect: &PotentialDialect,
) -> f64 {
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
            let boundary_count = boundary_counts.get_boundary_count(b'"', dialect.delimiter);
            if quote_counts.single == 0
                && boundary_count >= 2
                && double_density >= min_density_threshold
            {
                // No single quotes AND double quotes at boundaries with real density
                // This handles small files with quoted fields containing delimiters
                2.2
            } else if boundary_count >= 2 && double_density >= min_density_threshold {
                // Double quotes at boundaries with good density
                1.15
            } else if double_density >= min_density_threshold {
                // Double quotes have significant density - moderate boost
                1.08
            } else {
                // Neutral - rely on other scoring factors
                1.0
            }
        }
        Quote::Some(b'\'') => {
            // Single quotes are tricky because apostrophes are common in text
            // MUST have opening boundary evidence - apostrophes in content tend to appear
            // only before delimiters (closing only), while genuine quoting has both
            // opening (delimiter→quote) and closing (quote→delimiter) boundaries
            let boundary_count = boundary_counts.get_boundary_count(b'\'', dialect.delimiter);
            let opening_count =
                boundary_counts.get_single_opening_boundary_count(dialect.delimiter);
            if quote_counts.double == 0
                && opening_count >= 2
                && boundary_count >= 4
                && single_density >= min_density_threshold * 2
            {
                // No double quotes, opening+closing boundaries, high density
                // This is strong evidence of single-quote quoting
                2.2
            } else if quote_counts.double == 0
                && opening_count >= 1
                && boundary_count >= 2
                && single_density >= min_density_threshold
            {
                // No double quotes, opening boundary present, decent density
                1.20
            } else if double_density >= min_density_threshold {
                // Double quotes present - penalize single-quote detection
                0.90
            } else if boundary_count == 0 && single_density > 0 {
                // Single quotes present but not at boundaries - likely just text content
                // Slight penalty to prefer double-quote as default
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

/// Count opening quote boundaries (delimiter/newline → quote) only.
/// Used to distinguish genuine quoting from apostrophes that appear only at field ends.
fn quote_opening_boundary_count(data: &[u8], quote_char: u8, delimiter: u8) -> usize {
    let mut count = 0;
    for window in data.windows(2) {
        if (window[0] == delimiter || window[0] == b'\n' || window[0] == b'\r')
            && window[1] == quote_char
        {
            count += 1;
        }
    }
    // Also count start of data as an opening boundary
    if !data.is_empty() && data[0] == quote_char {
        count += 1;
    }
    count
}

/// Calculate quote evidence score using pre-computed counts and raw data for boundary detection.
/// This provides more accurate quote detection for small files.
fn quote_evidence_score_with_data(
    data: &[u8],
    quote_counts: &QuoteCounts,
    dialect: &PotentialDialect,
) -> f64 {
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
            let boundary_count = quote_boundary_count(data, b'"', dialect.delimiter);
            if quote_counts.single == 0
                && boundary_count >= 2
                && double_density >= min_density_threshold
            {
                // No single quotes AND double quotes at boundaries with real density
                // This handles small files with quoted fields containing delimiters
                2.2
            } else if boundary_count >= 2 && double_density >= min_density_threshold {
                // Double quotes at boundaries with good density
                1.15
            } else if double_density >= min_density_threshold {
                // Double quotes have significant density - moderate boost
                1.08
            } else {
                // Neutral - rely on other scoring factors
                1.0
            }
        }
        Quote::Some(b'\'') => {
            // Single quotes are tricky because apostrophes are common in text
            // MUST have opening boundary evidence - apostrophes in content tend to appear
            // only before delimiters (closing only), while genuine quoting has both
            // opening (delimiter→quote) and closing (quote→delimiter) boundaries
            let boundary_count = quote_boundary_count(data, b'\'', dialect.delimiter);
            let opening_count = quote_opening_boundary_count(data, b'\'', dialect.delimiter);
            if quote_counts.double == 0
                && opening_count >= 2
                && boundary_count >= 4
                && single_density >= min_density_threshold * 2
            {
                // No double quotes, opening+closing boundaries, high density
                // This is strong evidence of single-quote quoting
                2.2
            } else if quote_counts.double == 0
                && opening_count >= 1
                && boundary_count >= 2
                && single_density >= min_density_threshold
            {
                // No double quotes, opening boundary present, decent density
                1.20
            } else if double_density >= min_density_threshold {
                // Double quotes present - penalize single-quote detection
                0.90
            } else if boundary_count == 0 && single_density > 0 {
                // Single quotes present but not at boundaries - likely just text content
                // Slight penalty to prefer double-quote as default
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

        if score_ratio > 0.95 {
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
        // Pipe - common in data exports; intentionally tied with tab (both are
        // respectable standard delimiters); tie resolved by iteration order
        b'|' => 8,
        b':' => 4, // Colon - sometimes used, but also appears in timestamps
        b'^' => 3, // Caret - rare
        b'~' => 3, // Tilde - rare
        0xA7 => 2, // Section sign (§) - rare
        b'/' => 2, // Forward slash - rare
        b' ' => 2, // Space - very rare as delimiter, often appears in text
        b'#' => 1, // Hash - very rare, often used for comments
        b'&' => 1, // Ampersand - very rare
        _ => 0,    // Unknown delimiters - lowest priority
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

    // Get the list of delimiters being tested
    let delimiters: Vec<u8> = dialects
        .iter()
        .map(|d| d.delimiter)
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .collect();

    // Detect and normalize line endings once for all dialects
    // All dialects in the list have the same line terminator (set by detect_line_terminator)
    let line_terminator = dialects
        .first()
        .map_or(super::potential_dialects::LineTerminator::LF, |d| {
            d.line_terminator
        });
    let normalized_data = super::potential_dialects::normalize_line_endings(data, line_terminator);
    let normalized_bytes: &[u8] = normalized_data.as_ref();

    // Pre-compute quote boundary counts for all delimiters in one pass (on normalized data)
    let boundary_counts = QuoteBoundaryCounts::new(normalized_bytes, &delimiters);

    // Score all dialects in parallel, using per-thread reusable TypeScoreBuffers
    let pairs: Vec<(DialectScore, Table)> = dialects
        .par_iter()
        .map(|d| {
            BUFFERS.with(|b| {
                score_dialect_with_normalized_data(
                    normalized_bytes,
                    d,
                    max_rows,
                    &quote_counts,
                    &boundary_counts,
                    &mut b.borrow_mut(),
                )
            })
        })
        .collect();

    // Keep first-maximum semantics: when two dialects tie on gamma, the one
    // with the lower index (earlier in `dialects`) wins — matching the
    // original sequential `if score.gamma > best_gamma` loop which used
    // strict `>` so the first winner was never displaced by a tie.
    let best_table = pairs
        .iter()
        .enumerate()
        .max_by(|(i, a), (j, b)| {
            a.0.gamma
                .partial_cmp(&b.0.gamma)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| j.cmp(i)) // lower index wins on tie
        })
        .map(|(_, (_, t))| t.clone());

    let mut scores: Vec<DialectScore> = pairs.into_iter().map(|(s, _)| s).collect();

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

    // --- Tests for quote_opening_boundary_count and get_single_opening_boundary_count ---

    #[test]
    fn test_quote_opening_boundary_count_apostrophes_only() {
        // Apostrophes appear only before delimiters (closing-only), not at field starts
        // e.g. "value's, other" - apostrophe is mid-word, not at field start
        let data = b"value's, other's, thing's\n";
        let count = quote_opening_boundary_count(data, b'\'', b',');
        // No delimiter→quote or newline→quote or leading-quote transitions
        assert_eq!(count, 0);
    }

    #[test]
    fn test_quote_opening_boundary_count_genuine_quoting() {
        // Genuine single-quote quoting: quote appears at field start after delimiter/newline
        let data = b",'field', 'next'\n";
        let count = quote_opening_boundary_count(data, b'\'', b',');
        // First window [b',', b'\''] is delimiter→quote → +1 opening boundary
        // Second window [b' ', b'\''] is space→quote (space not a delimiter here) → 0
        assert!(
            count >= 1,
            "expected at least 1 opening boundary, got {count}"
        );
    }

    #[test]
    fn test_quote_opening_boundary_count_leading_quote() {
        // Data starts with the quote character = opening boundary
        let data = b"'field','next'\n";
        let count = quote_opening_boundary_count(data, b'\'', b',');
        // Starts with quote (+1), and delimiter→quote at position 7→8 (+1)
        assert_eq!(count, 2);
    }

    #[test]
    fn test_quote_opening_boundary_count_empty() {
        let count = quote_opening_boundary_count(b"", b'\'', b',');
        assert_eq!(count, 0);
    }

    #[test]
    fn test_get_single_opening_boundary_count_apostrophes_only() {
        // Apostrophes only at field ends (before delimiter) — no opening boundaries
        // "it's, we're, they've" — each apostrophe is mid-word, not at a field start
        let data = b"it's, we're, they've\n";
        let delimiters = vec![b','];
        let counts = QuoteBoundaryCounts::new(data, &delimiters);
        let opening = counts.get_single_opening_boundary_count(b',');
        assert_eq!(
            opening, 0,
            "apostrophes should produce zero opening boundaries"
        );
    }

    #[test]
    fn test_get_single_opening_boundary_count_genuine_quoting() {
        // Genuine single-quote quoting: 'val','val2' — quote appears right after delimiter
        let data = b"'first','second','third'\n";
        let delimiters = vec![b','];
        let counts = QuoteBoundaryCounts::new(data, &delimiters);
        let opening = counts.get_single_opening_boundary_count(b',');
        // data[0] == b'\'' counts via starts_with_single in get_boundary_count, but
        // get_single_opening_boundary_count only counts delimiter→quote and newline→quote,
        // plus data[0] if it is a quote (handled by starts_with_single bonus)
        assert!(
            opening >= 2,
            "expected ≥2 opening boundaries for genuinely quoted fields, got {opening}"
        );
    }
}

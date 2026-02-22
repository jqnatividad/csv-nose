# csv-nose Implementation Notes

csv-nose implements the **Table Uniformity Method (TUM)** from:

> García, W. S. (2024). *Detecting CSV File Dialects by Table Uniformity Measurement and Data Type Inference*. DOI: 10.13140/RG.2.2.28318.82245

This document describes the implementation pipeline, where we follow the paper closely, and where we diverge. The implementation was tuned to push benchmark accuracy above 90% on W3C-CSVW while maintaining or improving POLLOCK accuracy.

---

## 1. Pipeline Overview

The entry point is `Sniffer::sniff_bytes` in `src/sniffer.rs`. The pipeline is:

1. **Encoding detection + transcoding** — detect character encoding and transcode to UTF-8 if necessary; strip BOM
2. **Comment preamble stripping** — skip leading lines starting with `#` (with optional leading whitespace); count skipped rows
3. **Line terminator detection** — detect LF / CRLF / CR from the data (once, not per dialect)
4. **Dialect candidate generation** — 11 delimiters × 3 quote chars = 33 candidates, all sharing the detected line terminator (`src/tum/potential_dialects.rs`)
5. **Line ending normalization** — normalize to LF once before parallel scoring (zero-copy for LF files via `Cow::Borrowed`)
6. **Parallel dialect scoring** — score all 33 candidates via `rayon::par_iter` with thread-local `TypeScoreBuffers` (`src/tum/score.rs::score_all_dialects_with_best_table`)
7. **Best dialect selection** — `find_best_dialect` picks the winner with delimiter/quote priority tiebreaking
8. **Structural preamble detection** — identify non-data rows at the start using field count consistency
9. **Header detection** — multi-criterion heuristic scoring on the effective table
10. **Column type inference** — `infer_column_types` assigns a type to each column

---

## 2. Table Uniformity Scores

### tau_0 (Consistency)

Matches the paper exactly. Measures whether all rows have the same number of fields.

```
τ₀ = 1 / (1 + 2σ)
```

where σ is the standard deviation of per-row field counts. Range: [0, 1]; 1.0 = perfectly uniform.

Implementation: `src/tum/uniformity.rs::calculate_tau_0`

### tau_1 (Dispersion)

**Diverges from the paper.** The paper's formula is:

```
τ₁ = 2 · R(α² + 1)((1 - β) / M)
```

where R = range of field counts, α = number of row-to-row transitions, M = modal field count, β = M/n. This formula is unbounded (0 to ∞) and equals 0 for a perfectly uniform table.

Our formula is a bounded weighted composite, where 1.0 = perfectly uniform:

```
τ₁ = mode_score × 0.4 + range_score × 0.3 + transition_score × 0.3
```

- `mode_score` = fraction of rows with the modal field count
- `range_score` = `1 − (range / max_field_count)`, clamped to [0, 1]
- `transition_score` = `1 − (transitions / (n − 1))` where transitions counts row-to-row field count changes

**Rationale**: the paper's τ₁ is unbounded and grows with dispersion. Inverting and bounding it to [0, 1] lets it fit naturally into our gamma composite formula as a multiplicative term.

Implementation: `src/tum/uniformity.rs::calculate_tau_1`

---

## 3. Type Detection

**Type system** (`src/field_type.rs`, `src/tum/type_detection.rs`):

Types detected: `NULL`, `Unsigned`, `Signed`, `Float`, `Boolean`, `Date`, `DateTime`, `Text`

This is richer than the paper, which uses a binary known/unknown classification for scoring.

**Detection order** (optimized for hot path, each cell runs this sequence):

1. NULL — empty string or known null literals (`null`, `NA`, `N/A`, `NaN`, `#N/A`, `#VALUE!`, etc.)
2. Unsigned integer — direct digit scan, no regex; limited to 19 digits (fits u64)
3. Signed integer — direct scan for negative integers; positive integers are caught by Unsigned above
4. Boolean — exhaustive length-keyed match (`true`/`false`, `yes`/`no`, `on`/`off`, `1`/`0`, `y`/`n`, `t`/`f`)
5. Float — gated by `.contains('.')` or `.contains('e')` before applying float regex
6. DateTime — regex match for ISO 8601 and common timestamp formats
7. Date — regex match for common date formats
8. Text — fallthrough

The cheap string-operation gates for NULL, integers, and booleans avoid regex overhead on the most common cell types.

### Type Score

**Diverges from the paper.** The paper computes a global type score:

```
λ = (Σ Sᵢ)² / (100 · k²)
```

where Sᵢ = 100 if the cell type is "known", 0.1 if "unknown", and k = total cell count.

Our approach computes a **per-column consistency score**:

```
column_score = max_type_count / non_null_total
type_score = mean(column_score for all columns)
```

- `max_type_count` = count of the most frequent non-null type in the column
- NULL cells are excluded from the denominator (sparse data is not penalized)
- Final type score = average across all columns

**Rationale**: per-column scoring rewards uniform column types rather than raw "known type" fraction, which is a better signal for dialect detection (correct delimiter → consistent column types).

### Pattern Score

**Not in the paper.** An additional per-table pattern specificity score:

```
pattern_score = mean(specificity_weight for modal type of each column)
```

Type specificity weights: `DateTime`=1.0, `Date`=0.9, `Float`=1.0, `Unsigned`/`Signed`=1.0, `Boolean`=1.0, `Text`=0.1, null-like strings (`"NULL"`, `"NA"`, etc.)=0.5, empty string=0.0

Contributes 0.1× to gamma; rewards files with structured data types over free-form text.

Implementation: `src/tum/type_detection.rs::calculate_pattern_score`

---

## 4. Gamma Score (Combined Score)

**Diverges from the paper.** The paper's combined formula is:

```
ϖ = (τ₀/Δ + 1/(τ₁ + n)) · Σᵢ λᵢ   (for n > 1)
```

where Δ = expected record count threshold.

Our formula (`src/tum/score.rs::compute_gamma`):

```
uniformity_score = sqrt(τ₀ × τ₁)            # geometric mean
raw_score = uniformity_score × 0.5
          + type_score × 0.3
          + pattern_score × 0.1
          + row_bonus                         # up to +0.10 at ≥20 rows
          + field_bonus                       # up to +0.20 at ≥10 fields

gamma = raw_score
      × single_field_penalty   # 0.5× if modal_fields == 1
      × high_field_penalty     # 0.8× if >50 fields; 0.5× if >100 fields
      × delimiter_penalty      # 0.60–1.0 by delimiter rarity (see table below)
      × small_sample_penalty   # 0.80 if <3 rows; 0.90 if <5 rows; 1.0 otherwise
```

Then multiplied by `quote_multiplier` (0.90–2.2×) from quote evidence scoring (Section 5).

**Bonus details**:
- `row_bonus = (min(num_rows, 20) / 20) × 0.10`
- `field_bonus = (min(field_count, 10) / 10) × 0.20` (only when field_count ≥ 2)

**Delimiter penalty table**:

| Delimiter | Penalty | Relaxed condition |
|-----------|---------|-------------------|
| `,` `;` `\t` | 1.00 | — |
| `\|` | 0.98 | — |
| `:` | 0.90 | — *(vestigial — `:` is excluded from candidates; see Section 9)* |
| `^` `~` | 0.80 | — |
| `§` (0xA7) | 0.78 | — |
| ` ` | 0.75 | — |
| `/` | 0.65 | — |
| `#` | 0.60 | relaxed to 0.85 when ≥3 fields AND ≥50 rows |
| `&` | 0.60 | — |

**Rationale for divergence**: The paper's formula uses τ₀/Δ where Δ is the sample threshold — there is no equivalent concept in our implementation. We fold sample reliability into the row_bonus additive term and small_sample_penalty multiplicative term instead.

---

## 5. Quote Evidence Scoring

**Novel, not in the paper.** A major addition that significantly improves accuracy on quoted files.

Pre-computation (once per sniff call, not per dialect):

- **`QuoteCounts`**: total `"` and `'` character counts; `\"` and `\'` escape pair counts; total data length
- **`QuoteBoundaryCounts`**: opening boundaries (delimiter/newline → quote) and closing boundaries (quote → delimiter/newline) for each delimiter × quote combination, computed in a single pass

Quote density is measured in counts per 1000 bytes. The significance threshold is 5/1000 (0.5%).

### Double-quote multiplier rules (`Quote::Some(b'"')`)

| Condition | Multiplier |
|-----------|-----------|
| No single quotes + ≥2 boundaries + density ≥ 0.5% | **2.2×** |
| ≥2 boundaries + density ≥ 0.5% | **1.15×** |
| Density ≥ 0.5% only | **1.08×** |
| Otherwise | 1.0× |

### Single-quote multiplier rules (`Quote::Some(b'\'')`)

Opening boundary requirement guards against apostrophes in text content (which produce only closing boundaries before delimiters, never opening boundaries after them).

| Condition | Multiplier |
|-----------|-----------|
| No double quotes + ≥2 opening boundaries + ≥4 total boundaries + density ≥ 1.0% | **2.2×** |
| No double quotes + ≥1 opening boundary + ≥2 total boundaries + density ≥ 0.5% | **1.20×** |
| Double-quote density ≥ 0.5% (double quotes dominate) | **0.90×** |
| Backslash-escaped single quotes (`\'`) + no double-quote escapes + no boundaries | **1.10×** |
| No double quotes + 0 opening boundaries + ≥20 total boundaries + density ≥ 5% | **1.10×** |
| No boundaries at all + single quotes present | **0.95×** |
| Otherwise | 1.0× |

The closing-only 1.10× case handles hash-delimited data with space-padded fields (e.g., `# 'addr' # 'city'`) where the space between the `#` delimiter and the `'` quote character prevents the adjacency scan from detecting an opening boundary. The opening boundary scan requires the delimiter and quote to be immediately adjacent (no intervening whitespace), so `# '` registers as a closing boundary after the preceding field ends but not as an opening boundary for the next field.

### No-quote multiplier rules (`Quote::None`)

| Condition | Multiplier |
|-----------|-----------|
| Double-quote density ≥ 0.5% | **0.90×** |
| Otherwise | 1.0× |

### Special dampening rules

Applied after the base quote multiplier in `score_dialect_with_normalized_data`:

1. **JSON-like chaos**: if quote multiplier > 1.5, modal field count ≥ 5, table is non-uniform, first row has ≤1 field, and ≥3 distinct non-modal field counts exist → scale the boost excess down to 30%: `1.0 + (multiplier − 1.0) × 0.3`

2. **Space delimiter + empty first field**: if >50% of rows have an empty first field → cap quote multiplier at 1.05×, then apply 0.55× to the combined gamma. This suppresses the false quote boundary signal from spaces adjacent to quote characters in leading-space-padded row formats.

3. **Comma + ` # ` pattern**: if >90% of rows have ` # ` in the first parsed field AND comma yields exactly 2 fields → apply 0.82× to the comma dialect gamma. This indicates that `#` is the true delimiter and comma splits inside a `#`-delimited field.

---

## 6. Tiebreaking

**Extends the paper.** The paper's `GetBestDialect` simply picks the highest ϖ.

Our `find_best_dialect` (`src/tum/score.rs`):

- Compute `score_ratio = min_gamma / max_gamma` for each pair being compared
- If `score_ratio > 0.95` (scores within 5%), apply priority ordering:
  1. Delimiter priority (higher = preferred): `,`=10, `;`=9, `\t`=8, `|`=8, `:`=4 *(vestigial — excluded from candidates)*, `^`=3, `~`=3, `§`=2, `/`=2, ` `=2, `#`=1, `&`=1
  2. Quote priority (higher = preferred): `"`=3, `'`=2, `None`=1
  3. If both priorities tie, use raw gamma
- If all non-zero dialects produce single-field tables, apply priority ordering regardless of score gap (fallback for files that can't be parsed with any delimiter)

---

## 7. Preamble Detection

**Extends the paper** (not detailed there). Two-phase approach:

### Phase 1: Comment preamble (`src/sniffer.rs::skip_preamble`)

Strip leading lines that start with `#` (with optional leading whitespace/tabs). Performed before dialect scoring so comment lines don't pollute field count statistics. The count is stored and added to the final `Header.num_preamble_rows`.

### Phase 2: Structural preamble (`src/sniffer.rs::detect_structural_preamble`)

After dialect scoring, find the first row from which ≥80% of the remaining rows share the modal field count. Uses an O(n) suffix-count precomputation to avoid O(n²) scanning:

1. Compute the modal field count for the full table
2. Precompute `matching_suffix[i]` = number of rows from index i to end that match the modal count
3. Scan forward; return the first index i where `matching_suffix[i] / (n − i) ≥ 0.80`

Requires ≥3 rows to attempt detection. The total preamble count reported in metadata is `comment_rows + structural_rows`.

---

## 8. Header Detection

**Extends the paper.** The paper treats the header row as "a simple record." Our `detect_header` (`src/sniffer.rs`) uses a weighted multi-criterion score:

| Check | Score if true |
|-------|--------------|
| First row has more text-typed cells than second row | +1.0 |
| First row has more text cells than numeric cells | +0.5 |
| All first-row values are unique (no duplicate column names) | +0.5 |
| Average first-row cell length ≤ average second-row cell length | +0.3 |

`has_header = (total_score / 4) > 0.4`

Requires ≥2 rows. All type classification uses `detect_cell_type` from `src/tum/type_detection.rs`.

---

## 9. Candidate Dialects

**Extends the paper.** The paper tests `, ; TAB | SPACE` delimiters and `" ' ~` quote chars.

Our candidates (`src/tum/potential_dialects.rs`):

| Category | Values |
|----------|--------|
| Delimiters (11) | `,` `;` `\t` `\|` ` ` `^` `~` `#` `&` `§` `/` |
| Quote chars (3) | `"` `'` `None` |
| Line terminators | 1 per file (detected once, not iterated) |
| **Total candidates** | **33** |

Additional delimiters compared to paper: `#` (scientific/hash-delimited data), `§` (section sign, used in some European data formats), `/` (path-like delimiters), `^`, `~`.

Note: `:` (colon) is intentionally excluded from candidates despite appearing in `delimiter_priority` — it commonly appears in timestamp values and causes too many false positives.

---

## 10. Performance

Not described in the paper. Implemented to handle bulk sniffing efficiently:

- **Parallel scoring**: `rayon::par_iter` over all 33 candidates; each rayon worker thread owns a `TypeScoreBuffers` instance via `thread_local!` to avoid per-call heap allocation
- **Zero-copy normalization**: `normalize_line_endings` returns `Cow::Borrowed` for LF files (most common case)
- **Single-pass boundary counting**: `QuoteBoundaryCounts::new` scans data once for all delimiters using a 256-entry lookup table
- **Reused best table**: the parsed table for the winning dialect is passed through to preamble detection and metadata building, avoiding a redundant re-parse
- **`Cow<Table>` in build_metadata**: avoids cloning the table in the common no-preamble case

---

## Divergence Summary

| Aspect | Paper (García 2024) | Our Implementation |
|--------|---------------------|--------------------|
| τ₁ formula | `2·R(α²+1)((1−β)/M)`, unbounded, 0=uniform | Weighted composite, bounded [0,1], 1=uniform |
| Type score λ | Global `(ΣSᵢ)²/(100·k²)`, binary S | Per-column max-type consistency, NULL-excluded |
| Gamma formula | `(τ₀/Δ + 1/(τ₁+n))·Σλᵢ` | Weighted sum with multiplicative penalties |
| Quote detection | Not described | Layered density + boundary evidence multiplier |
| Tiebreaking | Highest score wins | Delimiter + quote priority within 5% score band |
| Preamble | Not detailed | Two-phase: comment lines + structural field-count analysis |
| Header | "Treat as a record" | Multi-criterion weighted heuristic |
| Delimiter set | `, ; TAB \| SPACE` (5) | 11 delimiters including `# § / ^ ~` |
| Pattern score | Not present | Type specificity score (0.1× weight in gamma) |
| Sample size | Threshold Δ parameter | `SampleSize::{Records(n), Bytes(n), All}` |
| Parallel scoring | Not described | Rayon par_iter + thread-local type score buffers |
| Line terminators | LF and CRLF iterated | Detected once, normalized before scoring |

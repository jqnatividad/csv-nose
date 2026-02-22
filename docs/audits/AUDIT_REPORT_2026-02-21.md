# Documentation Audit Report
Generated: 2026-02-21 | Commit: 88f752a

## Executive Summary

| Metric | Count |
|--------|-------|
| Documents scanned | 5 |
| Claims verified | ~120 |
| Verified TRUE | ~114 (95%) |
| **Verified FALSE** | **5 (4%)** |
| Unverifiable | ~1 (1%) |

Documents audited:
- `README.md`
- `src/lib.rs` (doc comment)
- `docs/IMPLEMENTATION.md`
- `docs/PERFORMANCE.md`
- `docs/BENCHMARK_DATASETS_INFO.md`
- `CHANGELOG.md`

> **Note:** `Cargo.toml edition = "2024"` was flagged by audit agents as invalid, but was verified to compile successfully. Rust 2024 edition was stabilized in Rust 1.85.0 (February 2025) and is valid.

---

## False Claims Requiring Fixes

### README.md

| Line | Claim | Reality | Fix |
|------|-------|---------|-----|
| 27 | `csv-nose = "0.6"` | `Cargo.toml` version is `0.8.0` | Change to `csv-nose = "0.8"` |

### src/lib.rs

| Line | Claim | Reality | Fix |
|------|-------|---------|-----|
| 44–46 | Cites "Wrangling Messy CSV Files by Detecting Row and Type Patterns" by van den Burg, Nazábal, and Sutton (2019) | The implemented algorithm is the **Table Uniformity Method** from "Detecting CSV File Dialects by Table Uniformity Measurement and Data Type Inference" by García (2024) | Replace citation with García (2024) paper |

### docs/IMPLEMENTATION.md

| Line | Claim | Reality | Fix |
|------|-------|---------|-----|
| 120 | `NULL`=0.0 type specificity weight | Empty strings return 0.0, but null-like strings (`"NULL"`, `"NA"`, `"N/A"`, etc.) match the `NULL_PATTERN` with weight **0.5** (`src/tum/regexes.rs:129`) | Change `NULL=0.0` to: empty string=0.0, null-like strings=0.5 |

### docs/BENCHMARK_DATASETS_INFO.md

| Line | Claim | Reality | Fix |
|------|-------|---------|-----|
| 93 | "Why CSV Wrangling has lower accuracy (~87%)" | Current accuracy is **92.74%** (as of 2026-02-21) | Update to ~93% or 92.74% |
| 100 | "Why POLLOCK is in between (96.62%)" | Current accuracy is **97.30%** (as of 2026-02-21) | Update to 97.30% |

---

## Outdated Documentation (Not False, But Stale)

### docs/PERFORMANCE.md

The document explicitly carries a `> **Note:** This document reflects csv-nose v0.4.0` header, but:
- Current version is 0.8.0 (v0.4.0 accuracy table is 4 minor versions behind)
- README.md links to it at line 213 without a staleness warning: *"See PERFORMANCE.md for details on accuracy breakdowns and known limitations"*
- Accuracy numbers have improved materially (POLLOCK: 96.62% → 97.30%, CSV Wrangling: 87.15% → 92.74%)

**Recommended action:** Either update PERFORMANCE.md to v0.8.0 accuracy data, or add a note in README.md that PERFORMANCE.md reflects historical v0.4.0 data.

---

## Pattern Summary

| Pattern | Count | Root Cause |
|---------|-------|------------|
| Outdated version reference | 2 | `README.md` dependency snippet not updated when cutting v0.8.0 |
| Wrong paper citation | 1 | `lib.rs` doc comment never updated to reflect García (2024) |
| Stale accuracy numbers | 2 | `BENCHMARK_DATASETS_INFO.md` written at v0.4.0 accuracy baseline |
| Outdated doc (intentional) | 1 | `PERFORMANCE.md` documents v0.4.0 behavior; linked without staleness warning |

---

## High-Confidence Verified Claims (Sample)

All of the following were checked against source code and confirmed correct:

- All 33 function/struct/method names in `docs/IMPLEMENTATION.md` ✓
- All delimiter penalty values (pipe=0.98, colon=0.90, space=0.75, hash=0.60, §=0.78, &=0.60, /=0.65) ✓
- All quote scoring multipliers (double 2.2×/1.15×/1.08×; single 2.2×/1.20×/1.10×/0.90×/0.95×) ✓
- Tiebreaking threshold 0.95 ✓
- All delimiter/quote priority values ✓
- All dataset file counts (POLLOCK=148, W3C-CSVW=221, CSV Wrangling=179, CODEC=142, MESSY=126) ✓
- All dataset delimiter/quote/encoding statistics ✓
- All CLI flags (`--benchmark`, `--annotations`, `-f json`, `--delimiter-only`) ✓
- All Cargo features (`http`) ✓
- All formula implementations (τ₀, τ₁, gamma, row_bonus, field_bonus) ✓
- Preamble consistency threshold ≥80% ✓
- Header scoring heuristics (+1.0/+0.5/+0.5/+0.3, threshold 0.4) ✓
- rayon `par_iter` with thread-local `TypeScoreBuffers` ✓
- `Cow<Table>` zero-clone optimization ✓
- 32KB CSV reader buffer capacity ✓
- `normalize_line_endings` returns `Cow::Borrowed` for LF files ✓

---

## Human Review Queue

- [ ] `docs/PERFORMANCE.md`: Decide whether to update to v0.8.0 accuracy data or add a README staleness warning
- [ ] `src/lib.rs` lines 44–46: Confirm García (2024) is the sole/primary reference (or whether van den Burg should also be cited as related work)

# Accuracy and Known Limitations

This document describes cases where csv-nose may not correctly detect CSV dialects, helping you understand when to use manual overrides.

## Accuracy Summary

Tested against standard CSV benchmark datasets:

| Dataset | Success Rate | Notes |
|---------|--------------|-------|
| POLLOCK | 98.65% | General CSV files |
| W3C-CSVW | 99.55% | W3C CSV on the Web test suite |
| CSV Wrangling | 94.97% | Real-world messy CSVs |
| CSV Wrangling CODEC | 94.37% | Filtered subset |
| CSV Wrangling MESSY | 93.65% | Non-normal structures |

## Known Limitations

### Uncommon Delimiters

csv-nose is biased toward common delimiters (`,`, `;`, `\t`) to improve accuracy on real-world data. Files using rare delimiters may be misdetected.

**Space-delimited files** (0.75 penalty, 0.45 below 5 rows):
- Spaces appear frequently in text content, making them difficult to distinguish as delimiters
- A table of only a few rows does not provide enough repetition to tell a real
  space delimiter from incidental spacing, so space is penalized harder there.
  Genuine space-delimited files are column-aligned dumps with many rows.
- Still challenging: `methane_molecular_structure_xyz_20140911.csv`, which uses
  *runs* of spaces for column alignment; runs are not collapsed into a single
  delimiter, so each run parses as several empty fields

**Hash-delimited files** (0.60 penalty):
- Hash (`#`) is commonly used as a comment marker
- Examples: `councils.csv`, `flat_file_database.csv`, `uniq_nl_data.csv`

**Other rare delimiters**:
- Ampersand (`&`): 0.60 penalty
- Forward slash (`/`): 0.65 penalty
- Section sign (`§`): 0.78 penalty
- Caret (`^`) and tilde (`~`): 0.80 penalty
- Colon (`:`): 0.90 penalty (often appears in timestamps)

**Workaround**: Use explicit delimiter hint:
```rust
use csv_nose::Sniffer;

let metadata = Sniffer::new()
    .delimiter(b' ')  // Force space delimiter
    .sniff_path("space-delimited.csv")?;
```

### Quote Character Detection

**Single-quote vs double-quote**:
- Quote detection now uses boundary analysis - quotes must appear at field boundaries (after delimiter/newline or before delimiter/newline) to receive a boost
- Single quotes require boundary evidence AND no double quotes present to be detected
- Single quotes appearing only within text content (not at boundaries) receive a 0.95 penalty
- When double quotes are present, single-quote dialects receive a 0.90 penalty
- Examples of challenging files: `Auto_Tone_sub315_day1.csv`, `currencies.csv`, `isco.csv`

**Quote::None when quotes exist**:
- When double quotes have ≥0.5% density, `Quote::None` receives a 0.90 penalty
- This helps prefer quoted parsing when evidence exists

**Workaround**: Use explicit quote hint:
```rust
use csv_nose::{Sniffer, Quote};

let metadata = Sniffer::new()
    .quote(Quote::Some(b'\''))  // Force single quote
    .sniff_path("single-quoted.csv")?;
```

### Small Files

Files with few rows have less reliable detection:

| Rows | Penalty |
|------|---------|
| < 3 | 0.80 |
| 3-4 | 0.90 |
| ≥ 5 | None |

**Workaround**: Increase sample size or provide hints:
```rust
use csv_nose::{Sniffer, SampleSize};

let metadata = Sniffer::new()
    .sample_size(SampleSize::All)  // Read entire file
    .sniff_path("small.csv")?;
```

### Preamble Handling

Non-tabular lines above the real table are handled in two places:

- **Comment preamble** — leading `#` lines are stripped *before* dialect
  scoring, so they cannot distort delimiter detection. Blank lines *inside* a
  leading comment block are tolerated; a trailing run of blanks that is followed
  by data is left in place.
- **Structural preamble** — a candidate whose field-count variance comes
  entirely from a non-tabular leading block (for example `$$section` /
  `key=[...]` metadata above a real table) is scored on the tabular part only.
  Guard rails keep this from becoming a general "discard inconvenient rows"
  escape hatch: at least 2 and at most 25% of rows may be discarded, each
  discarded row must hold under 25% of the modal field count, and at least 5
  rows must survive.

Comments *interspersed throughout* a file (rather than in a leading block) are
still treated as data. A file whose comment lines are scattered between data
rows may be misdetected.

### Files That Are Not Really CSV

Some benchmark failures are not detection bugs. They are recorded here so they
are not repeatedly re-investigated:

| File | Annotated | Detected | Why it is not worth forcing |
|------|-----------|----------|------------------------------|
| `gsi_adresser_og_elevtall_2010.csv` | `,` | `;` | The file is plainly `;`-delimited (7 fields, 3572 rows). The annotation looks mislabeled — csv-nose is arguably correct. |
| `bugs.csv` | `,` | `;` | An HTTP response dump (`HTTP/1.1 200 OK` plus header lines), not tabular data. |
| `replace.csv` | `,` | `;` | Tab/`;` fragments with no commas at all. |
| `admins.csv` | `,` | `#` | Effectively single-column (`milan#_%pass`). |
| `BIO.csv`, `Empty.csv` | `,` | `;` | LimeSurvey dumps behind a `#` comment header. |
| `register_data.csv` | `;` | `\t` | Four lines, of which exactly one is data. Neither preamble rule can reach it: line 1 (`SEQUENTIAL`) is not a comment, and structural detection needs at least 3 rows. |
| `task4_pad.csv` | `,` | ` ` | A Xilinx PAD report — a fixed-width text report rather than a delimited file. |

Forcing these would mean either trusting a mislabeled annotation or
special-casing degenerate input, at the cost of regressing legitimate files.

### Multi-table and Embedded Content

Files containing multiple tables or embedded non-CSV content may confuse detection:
- `file_multitable_less.csv`
- `file_multitable_more.csv`
- `file_multitable_same.csv`

These files have ambiguous structure where multiple dialects produce similar uniformity scores.

### Extreme Field Counts

**Single field** (0.50 penalty):
- A single field per row usually indicates the wrong delimiter was selected

**Very high field counts**:
- 50-100 fields: 0.80 penalty
- \>100 fields: 0.50 penalty
- May indicate splitting on a character that appears frequently in content

## Scoring Algorithm Reference

### Delimiter Penalties

| Delimiter | Penalty | Priority (tiebreaker) |
|-----------|---------|----------------------|
| `,` `;` `\t` | 1.00 | 10, 9, 8 |
| `\|` | 0.98 | 8 (ties with `\t`) |
| `:` | 0.90 | 4 |
| `^` `~` | 0.80 | 3 |
| `§` | 0.78 | 2 |
| ` ` (space) | 0.75 (0.45 if < 5 rows) | 2 |
| `/` | 0.65 | 2 |
| `#` `&` | 0.60 | 1 |

When scores are within 5%, delimiter priority is used as a tiebreaker.

### Delimiters Inside Quoted Fields

A candidate delimiter whose occurrences all fall *inside* double-quoted (`"..."`)
regions is demoted (0.50 penalty). The `csv` reader only honours `"` as a quote
when it opens a field, so a delimiter appearing inside a mid-field quoted value
(e.g. `;` in `Field1,Field2,"Field;3;3;3"`) would otherwise split the value into
spurious extra fields and beat the true delimiter. The penalty fires only when the
candidate actually split into ≥2 fields on those inside-quote delimiters, so a
correctly-quoted single-column value like `"123,,456.789"` (which parses to one
field under `,`+`"`) is never affected.

### Quote Evidence Scoring

Quote detection uses boundary analysis (quotes appearing at field boundaries, e.g., after/before the delimiter or newline) for improved accuracy:

| Condition | Multiplier |
|-----------|------------|
| Double quotes at boundaries, no single quotes | 2.20 boost |
| Double quotes at boundaries with good density | 1.15 boost |
| Double quotes with ≥0.5% density | 1.08 boost |
| Single quotes at boundaries (≥4), no double quotes, high density | 2.20 boost |
| Single quotes at boundaries (≥2), no double quotes | 1.20 boost |
| Single quote dialect when double quotes present | 0.90 penalty |
| Single quotes present but not at boundaries | 0.95 penalty |
| Quote::None when double quotes have ≥0.5% density | 0.90 penalty |

## Workarounds Summary

```rust
use csv_nose::{Sniffer, Quote, SampleSize};

// Force specific delimiter
let metadata = Sniffer::new()
    .delimiter(b'#')
    .sniff_path("hash-delimited.csv")?;

// Force specific quote character
let metadata = Sniffer::new()
    .quote(Quote::Some(b'\''))
    .sniff_path("single-quoted.csv")?;

// Force no quoting
let metadata = Sniffer::new()
    .quote(Quote::None)
    .sniff_path("unquoted.csv")?;

// Read entire file instead of sampling
let metadata = Sniffer::new()
    .sample_size(SampleSize::All)
    .sniff_path("small.csv")?;

// Combine hints
let metadata = Sniffer::new()
    .delimiter(b' ')
    .quote(Quote::None)
    .sample_size(SampleSize::Records(1000))
    .sniff_path("tricky.csv")?;
```

## When to Use Alternative Approaches

Consider using explicit dialect specification (bypassing sniffing entirely) when:

1. **You know the dialect** - If your data source has a documented format
2. **Consistent pipeline** - Processing files from the same source repeatedly
3. **Rare delimiters** - Space, hash, or other uncommon separators
4. **Performance critical** - Sniffing adds overhead; known formats can skip detection

For these cases, use a CSV parser directly with explicit configuration rather than sniffing.

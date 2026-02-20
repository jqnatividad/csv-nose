---
name: benchmark
description: Run csv-nose benchmark accuracy tests against POLLOCK, W3C-CSVW, or CSV Wrangling datasets
---

# Benchmark Skill

Run benchmark accuracy tests. Accept an optional dataset argument: `pollock`, `w3c`, `wrangling`, `codec`, `messy`, or `all` (default).

## Commands

| Dataset | Command |
|---------|---------|
| pollock | `cargo run --release -- --benchmark tests/data/pollock --annotations tests/data/annotations/pollock.txt` |
| w3c | `cargo run --release -- --benchmark tests/data/w3c-csvw --annotations tests/data/annotations/w3c-csvw.txt` |
| wrangling | `cargo run --release -- --benchmark tests/data/csv-wrangling --annotations tests/data/annotations/csv-wrangling.txt` |
| codec | `cargo run --release -- --benchmark tests/data/csv-wrangling --annotations tests/data/annotations/csv-wrangling-codec.txt` |
| messy | `cargo run --release -- --benchmark tests/data/csv-wrangling --annotations tests/data/annotations/csv-wrangling-messy.txt` |
| all | Run all of the above sequentially |

## Instructions

1. If no dataset is specified, run all benchmarks sequentially
2. After running, compare accuracy numbers against the benchmark tables in README.md
3. Flag any regressions (accuracy drops compared to README values)
4. Summarize results in a table showing: dataset, files tested, delimiter accuracy, quotechar accuracy, overall accuracy

---
name: cargo-audit
description: Check dependencies for known security vulnerabilities using cargo-audit
disable-model-invocation: true
---

# Cargo Audit

Run `cargo audit` to check for security vulnerabilities in dependencies.

## Steps
1. Check if cargo-audit is installed: `cargo audit --version 2>/dev/null`
   - If not installed: `cargo install cargo-audit`
2. Run: `cargo audit`
3. Summarize any advisories found (ID, crate, description, severity)
4. If no advisories: confirm all dependencies are clean

## Output
Report advisory count, severity levels, and whether any affected crates are
in the direct dependency list vs transitive (check Cargo.toml to distinguish).

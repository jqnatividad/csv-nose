---
name: release-prep
description: Prepare a new release - update version, changelog, run full test suite and benchmarks
disable-model-invocation: true
---

# Release Prep

Prepare a new csv-nose release. Takes an optional version number argument.

## Steps

1. If no version number provided, ask for one
2. Show current version from Cargo.toml
3. Update version in Cargo.toml
4. Update CHANGELOG.md with changes since the last release tag (use `git log` to find changes)
   - Follow the existing Keep a Changelog format (Performance, Changed, Fixed, Added sections)
   - Include the Full Changelog comparison link
5. Run quality checks:
   - `cargo fmt --check`
   - `cargo clippy`
   - `cargo test`
6. Run full benchmark suite (all 5 datasets) and compare results with README.md accuracy tables
7. Run `cargo package --list` to verify publish contents look correct
8. Summarize all results and flag any issues before the user publishes
9. Create a GitHub draft release using `gh release create`:
   - Tag: `v{version}`
   - Title: `v{version}`
   - Use `--draft` so the user can review before publishing
   - Pass the CHANGELOG.md entry for this version as release notes via `--notes-file`. Extract it with `awk` into a temp file you delete afterwards:
     ```bash
     VERSION="0.9.0"  # ← replace this with the actual version
     NOTES=$(mktemp)
     awk -v pre="## [$VERSION]" 'index($0,pre)==1{f=1;next} f && /^## \[/{exit} f && (NF||seen){seen=1;print}' CHANGELOG.md > "$NOTES"
     [ -s "$NOTES" ] || { echo "ERROR: no CHANGELOG section found for $VERSION"; rm -f "$NOTES"; exit 1; }
     gh release create "v$VERSION" --draft --title "v$VERSION" --notes-file "$NOTES"
     rm -f "$NOTES"
     ```
     Three things this gets right, each of which was previously wrong:
     - **Bracketed heading, no `v`.** This project's CHANGELOG headings look like `## [1.3.0] - 2026-09-03`, while the git tags are `v1.3.0`. A pattern like `## v$VERSION` matches nothing and hands `gh` an **empty release body without erroring** — which is exactly what happened during the 1.3.0 release. The `-s` guard is what turns that silent failure into a loud one; do not drop it.
     - **`index()` rather than a regex**, so the dots in `1.3.0` are matched literally instead of as wildcards.
     - **The heading line is skipped** (`next`, not `print`) since the release title already carries the version, and leading blank lines are trimmed — matching how the published v1.2.0 and earlier release bodies read.

     Do NOT use `--generate-notes` (that pulls from PR history, not CHANGELOG).
   - Before running `gh release create`, ensure the tag exists and is pushed. Create an annotated tag if it doesn't exist locally, then push it:
     ```
     git tag -a v{version} -m "v{version}" 2>/dev/null || true
     git push origin v{version} 2>/dev/null || true
     ```
     This handles three cases: tag doesn't exist (creates it), tag exists locally but not remotely (pushes it), tag exists remotely (push rejected by remote is safely ignored via `|| true`). This prevents `gh` from creating a lightweight tag pointing to the wrong commit.

10. **Do NOT run `cargo publish`** — leave that to the user

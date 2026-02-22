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
   - Pass the CHANGELOG.md entry for this version as release notes using `--notes-file` (write the entry to a temp file and delete it afterwards) or inline via `--notes`. For the inline approach, set `VERSION` to the actual version number (e.g. `0.9.0`) before running:
     ```bash
     VERSION="0.9.0"  # ← replace this with the actual version
     --notes "$(awk "/## v$VERSION/{f=1; print; next} f && /## v[0-9]/{exit} f" CHANGELOG.md)"
     ```
     If using `--notes-file`, remember to delete the temp file after the `gh release create` command completes.
     Do NOT use `--generate-notes` (that pulls from PR history, not CHANGELOG).
   - Before running `gh release create`, ensure the tag exists and is pushed. Create an annotated tag if it doesn't exist locally, then push it:
     ```
     git tag -a v{version} -m "v{version}" 2>/dev/null || true
     git push origin v{version} 2>/dev/null || true
     ```
     This handles three cases: tag doesn't exist (creates it), tag exists locally but not remotely (pushes it), tag exists remotely (push rejected by remote is safely ignored via `|| true`). This prevents `gh` from creating a lightweight tag pointing to the wrong commit.

10. **Do NOT run `cargo publish`** — leave that to the user

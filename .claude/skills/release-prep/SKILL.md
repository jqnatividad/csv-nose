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
   - Pass the CHANGELOG.md entry for this version as release notes using `--notes-file` (write the entry to a temp file) or `--notes "$(awk ...)"` — do NOT use `--generate-notes` (that pulls from PR history, not CHANGELOG)
   - Before running `gh release create`, ensure the tag exists and is pushed: `git tag v{version} && git push origin v{version}`. If the tag already exists remotely, skip creation. This prevents `gh` from creating a lightweight tag pointing to the wrong commit.

10. **Do NOT run `cargo publish`** — leave that to the user

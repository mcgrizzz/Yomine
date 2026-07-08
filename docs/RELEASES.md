# Release Guide

## Cutting a Release

1. Bump the version — one line: `[workspace.package] version` in the root `Cargo.toml`
   (both crates and the Tauri bundle inherit it)
2. Push/merge to `main` and let the **Tests** workflow pass
3. Go to **Actions → Manual Release → Run workflow**
4. Select release type: `stable` or `beta`
5. Check "Create release"
6. Click "Run workflow"

There is no version input — the tag is computed from `Cargo.toml`:

- **Stable**: `v1.0.0`
- **Beta**: `v1.0.0-beta.N` (N auto-increments across betas of the same version)

### Build-only dry run

Run Manual Release with "Build binaries without creating release" checked to get the
installers as workflow artifacts without tagging or releasing anything.

## What Happens Automatically

- Verifies the **Tests** workflow passed for the current commit
- Computes the version/tag and fails if that tag already exists
- Creates the git tag and a **draft** release — invisible to users and to the in-app
  updater until everything below is uploaded
- `release.yml` builds Windows/macOS/Linux installers via tauri-action and uploads them,
  plus the signed updater artifacts (`latest.json`, `.sig`) that power in-app updates
- A consolidated `SHA256SUMS.txt` is attached
- The draft is published — only now do `/releases/latest` and the updater see it
- Release notes are generated from `.github/release-notes-template.md` plus the GitHub
  changelog since the previous release (previous stable for stables; previous beta of the
  same version, or last stable, for betas)

### If a build fails partway

The draft release and the git tag already exist, so re-running Manual Release would fail
on the duplicate tag. Instead, fix the problem and run the **Release** workflow directly
(Actions → Release → Run workflow) with the same tag — it uploads into the existing draft
and publishes it at the end.

## Required Secrets

- `TAURI_SIGNING_PRIVATE_KEY` / `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` — updater artifact
  signing; builds fail without them (`bundle.createUpdaterArtifacts` is on). The private
  key must stay backed up — losing it orphans every installed copy.
- `MY_PAT` — used to create the release so the `release` event triggers the build and
  release-notes workflows (events from `GITHUB_TOKEN` don't trigger other workflows).

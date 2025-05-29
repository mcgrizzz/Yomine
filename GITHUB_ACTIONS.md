# GitHub Actions Workflows

This repository includes several GitHub Actions workflows for automated building and testing:

## Workflows

### 1. Beta Builds (`build.yml`)
- **Trigger**: Pushes to the `main` branch
- **Purpose**: Creates beta builds for testing
- **Artifacts**: Produces binaries for Windows, macOS (Intel & ARM), and Linux
- **Output**: Creates a prerelease with beta builds attached

### 2. Release Builds (`release.yml`)
- **Trigger**: When a new release is created on GitHub
- **Purpose**: Creates official release binaries
- **Artifacts**: Produces archived binaries for all platforms
- **Output**: Attaches binaries to the GitHub release

### 3. Tests (`test.yml`)
- **Trigger**: Pull requests and pushes to `main`
- **Purpose**: Runs tests and formatting checks
- **Platforms**: Ubuntu Linux

The test workflow:
1. **Sets up the environment**: Installs Rust nightly (for rustfmt), Linux dependencies for egui, and creates basic directory structure
2. **Runs code quality checks**: 
   - `cargo fmt --check` for formatting (using nightly rustfmt)
3. **Runs tests**: Executes the test suite, excluding tests that require large external dependencies:
   - Skips `dictionary::frequency_manager::tests::test_frequency` (requires frequency dictionary files)
   - Skips `segmentation::rule_matcher_tests::tests::inspect_tokens` (requires tokenizer dictionary files)

**Note**: Some tests require external dictionary files that would be too large to download in CI. These tests are skipped in the CI environment but can be run locally when the appropriate dictionaries are available.

## Creating a Release

To create a new release with binaries:

1. Go to your repository on GitHub
2. Click "Releases" → "Create a new release"
3. Create a new tag (e.g., `v0.1.0`)
4. Fill in the release title and description
5. Click "Publish release"

The release workflow will automatically build and attach binaries for all supported platforms.

## Beta Builds

Every push to the `main` branch automatically creates a beta release with the latest binaries. These are marked as prereleases and are useful for testing the latest changes.

# Release Guide

### Creating a Beta Release
1. Update version in `Cargo.toml`: `version = "1.0.0"`
2. Go to **Actions → Manual Release → Run workflow**
3. Enter version: `v1.0.0-beta.1`
4. Select release type: `beta`
5. Check "Create release"
6. Click "Run workflow"

### Creating a Stable Release
1. Update version in `Cargo.toml`: `version = "1.0.0"`
2. Go to **Actions → Manual Release → Run workflow**
3. Enter version: `v1.0.0`
4. Select release type: `stable`
5. Check "Create release"
6. Click "Run workflow"

## Version Format
- **Stable**: `v1.0.0`, `v1.2.3`, `v2.0.0`
- **Beta**: `v1.0.0-beta.1`, `v1.0.0-beta.2`

## What Happens Automatically
- **Verifies CI tests passed** for the current commit
- Validates version format and availability
- Creates the GitHub release page
- **Triggers automatic binary building** via release.yml workflow
- Generates release notes with contributor lists  
- Uploads binaries with checksums

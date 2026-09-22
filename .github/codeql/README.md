# CodeQL

CodeQL scans relevant pull request updates, nightly at 07:23 UTC, and on manual dispatch. It does not run on pushes to `main`. Main-branch alerts and direct pushes are therefore checked on the nightly schedule. New commits cancel older scans for the same PR.

The filter checks the entire PR diff. Documentation, standalone tests, and workspace-version-only changes skip analysis; production code, dependencies, and workflows still trigger it. Skips happen inside the workflow so its checks can finish. Edits to inline Rust tests still trigger a scan because they share files with production code.

`config.yml` excludes standalone tests. The Rust extractor's `cargo_cfg_overrides=-test` setting excludes inline `#[cfg(test)]` code. All four existing languages and their default security queries remain enabled.

## Switching from default setup

Review this branch before changing repository settings. GitHub's default setup overrides custom CodeQL workflows and rejects their result uploads while it is enabled.

1. Once the change is approved, switch off default setup in **Settings → Advanced Security → CodeQL analysis → Switch to advanced → Disable CodeQL**. Use this workflow rather than committing another generated workflow.
2. Merge this branch. Under **Actions → CodeQL**, enable the workflow if GitHub disabled it, then **Run workflow** on `main` to establish the new baseline immediately.
3. Verify all four analyses complete and subsequent PR scans compare against that baseline. If GitHub still reports a missing **Default setup** configuration, remove that inactive configuration from the code scanning tool status page after verifying the replacement.

No repository settings are changed by committing this workflow. Before the switch, default setup will continue its existing scans and advanced result uploads will not work.

See [GitHub's migration instructions](https://docs.github.com/en/code-security/how-tos/find-and-fix-code-vulnerabilities/configure-code-scanning/configuring-advanced-setup-for-code-scanning) and [Rust extractor options](https://github.com/github/codeql/blob/main/rust/codeql-extractor.yml).

## Validation

Run `node --test .github/scripts/codeql-filter.test.js` and `actionlint`. The Rust exclusions were also checked with CodeQL 2.27.0 against a small Cargo fixture: the default database included inline and integration test functions; the configured database retained production functions, including `#[cfg(not(test))]`, and excluded both test functions. Full repository scan timing and GitHub uploads need verification after switching setups.

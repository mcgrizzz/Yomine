const assert = require('node:assert/strict');
const { test } = require('node:test');
const { shouldAnalyze } = require('./codeql-filter.js');
const { inspectVersionBump } = require('./version-only.js');

const version = {
  filename: 'Cargo.toml', status: 'modified', additions: 1, deletions: 1,
  patch: '@@ -12,3 +12,3 @@\n [workspace.package]\n-version = "0.7.3"\n+version = "0.7.4"',
};
const analyze = (...files) => shouldAnalyze(files, files.length);

test('skips documentation and standalone tests, including mixed changes', () => {
  const paths = ['README.md', 'tests/segmentation.rs', 'src-tauri/ui/tests/possible-matches.mjs',
    '.github/scripts/codeql-filter.test.js', 'src/segmentation/rule_matcher_tests.rs',
    'research/lexical/test_build.py', 'ui/__tests__/example.ts', 'ui/example.spec.ts'];
  assert.equal(analyze(...paths.map(filename => ({ filename }))), false);
});

test('scans production code, dependencies, workflows, and mixed PRs', () => {
  for (const filename of ['src/anki/api.rs', 'Cargo.lock', 'Cargo.toml',
    'src-tauri/ui/pnpm-lock.yaml', '.github/workflows/test.yml', '.github/scripts/version-only.js']) {
    assert.equal(analyze(version, { filename }), true, filename);
  }
});

test('skips a complete workspace version diff, with CRLF and accompanying docs', async () => {
  assert.equal(analyze(version), false);
  assert.equal(analyze({ ...version, patch: version.patch.replaceAll('\n', '\r\n') },
    { filename: 'CHANGELOG.md' }), false);
  const github = { rest: { repos: { getCommit: async () => ({
    data: { files: [version], parents: [{ sha: 'parent' }] },
  }) } } };
  assert.deepEqual(await inspectVersionBump({ github, context: { repo: {} }, sha: 'bump' }),
    { isVersionOnly: true, parent: 'parent' });
});

test('does not mistake dependency versions or incomplete patches for release bumps', () => {
  for (const patch of [undefined,
    version.patch.replace('[workspace.package]', '[dependencies.example]'),
    version.patch.replace('[workspace.package]', '[workspace.package]\n [dependencies.example]'),
    version.patch.replace('[workspace.package]', '[workspace.package]\n@@ -90,2 +90,2 @@'),
  ]) assert.equal(analyze({ ...version, patch }), true);
  assert.equal(analyze({ ...version, additions: 2, deletions: 2 }), true);
  assert.equal(analyze({ ...version, status: 'added' }), true);
});

test('scans renames into or out of excluded paths', () => {
  assert.equal(analyze({ filename: 'tests/former-production.rs', previous_filename: 'src/lib.rs' }), true);
  assert.equal(analyze({ filename: 'src/lib.rs', previous_filename: 'tests/helper.rs' }), true);
  assert.equal(analyze({ filename: 'tests/new.rs', previous_filename: 'tests/old.rs' }), false);
});

test('scans when the PR file list is empty or incomplete', () => {
  assert.equal(shouldAnalyze([], 0), true);
  assert.equal(shouldAnalyze([{ filename: 'README.md' }], 3001), true);
  assert.equal(shouldAnalyze([version], undefined), true);
});

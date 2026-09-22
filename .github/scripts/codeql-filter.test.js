const assert = require('node:assert/strict');
const { test } = require('node:test');
const { ALL_LANGUAGES, selectLanguages } = require('./codeql-filter.js');
const { inspectVersionBump } = require('./version-only.js');

const version = {
  filename: 'Cargo.toml', status: 'modified', additions: 1, deletions: 1,
  patch: '@@ -12,3 +12,3 @@\n [workspace.package]\n-version = "0.7.3"\n+version = "0.7.4"',
};
const select = (...files) => selectLanguages(files, files.length);

test('skips documentation and standalone tests, including mixed changes', () => {
  const paths = ['README.md', 'tests/segmentation.rs', 'src-tauri/ui/tests/possible-matches.mjs',
    '.github/scripts/codeql-filter.test.js', 'src/segmentation/rule_matcher_tests.rs',
    'research/lexical/test_build.py', 'ui/__tests__/example.ts', 'ui/example.spec.ts'];
  assert.deepEqual(select(...paths.map(filename => ({ filename }))), []);
});

test('selects the affected language for source and dependency changes', () => {
  const cases = [
    [['src/anki/api.rs', 'Cargo.lock', 'src-tauri/Cargo.toml', '.cargo/config.toml', 'rust-toolchain.toml'], ['rust']],
    [['src-tauri/ui/src/lib/components/AboutModal.svelte', 'src-tauri/ui/src/app.d.ts',
      'src-tauri/ui/vite.config.ts', 'src-tauri/ui/pnpm-lock.yaml', 'src-tauri/ui/package.json'], ['javascript-typescript']],
    [['research/lexical/build.py', 'research/requirements.txt', 'pyproject.toml'], ['python']],
    [['.github/workflows/test.yml', '.github/actions/example/action.yml'], ['actions']],
    [['.github/scripts/test-build.js'], ['actions', 'javascript-typescript']],
    [['src-tauri/tauri.conf.json', 'src-tauri/capabilities/default.json'], ['javascript-typescript', 'rust']],
    [['.github/workflows/codeql.yml', '.github/codeql/config.yml', '.github/scripts/codeql-filter.js',
      '.github/scripts/version-only.js', 'unknown.config'], ALL_LANGUAGES],
  ];
  for (const [paths, languages] of cases) {
    for (const filename of paths) assert.deepEqual(select(version, { filename }), languages, filename);
  }
});

test('frontend-only PRs skip Rust, while mixed PRs include both languages', () => {
  const frontend = ['src-tauri/ui/src/app.d.ts', 'src-tauri/ui/src/lib/components/AboutModal.svelte',
    'src-tauri/ui/vite.config.ts'].map(filename => ({ filename }));
  assert.deepEqual(select(...frontend), ['javascript-typescript']);
  assert.deepEqual(select(...frontend, { filename: 'src/lib.rs' }), ['javascript-typescript', 'rust']);
});

test('skips a complete workspace version diff, with CRLF and accompanying docs', async () => {
  assert.deepEqual(select(version), []);
  assert.deepEqual(select({ ...version, patch: version.patch.replaceAll('\n', '\r\n') },
    { filename: 'CHANGELOG.md' }), []);
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
  ]) assert.deepEqual(select({ ...version, patch }), ['rust']);
  assert.deepEqual(select({ ...version, additions: 2, deletions: 2 }), ['rust']);
  assert.deepEqual(select({ ...version, status: 'added' }), ['rust']);
});

test('scans renames into or out of excluded paths', () => {
  assert.deepEqual(select({ filename: 'tests/former-production.rs', previous_filename: 'src/lib.rs' }), ['rust']);
  assert.deepEqual(select({ filename: 'src/lib.rs', previous_filename: 'tests/helper.rs' }), ['rust']);
  assert.deepEqual(select({ filename: 'tests/new.rs', previous_filename: 'tests/old.rs' }), []);
  assert.deepEqual(select({ filename: 'src-tauri/ui/helper.ts', previous_filename: 'src/helper.rs' }),
    ['javascript-typescript', 'rust']);
});

test('scans when the PR file list is empty or incomplete', () => {
  assert.deepEqual(selectLanguages([], 0), ALL_LANGUAGES);
  assert.deepEqual(selectLanguages([{ filename: 'README.md' }], 3001), ALL_LANGUAGES);
  assert.deepEqual(selectLanguages([version], undefined), ALL_LANGUAGES);
});

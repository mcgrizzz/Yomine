const { isWorkspaceVersionOnly } = require('./version-only.js');

const ALL_LANGUAGES = ['actions', 'javascript-typescript', 'python', 'rust'];

function isExcluded(path) {
  return /\.(md|rst)$/i.test(path) || path === 'LICENSE' ||
    /(^|\/)(tests|__tests__)\//.test(path) ||
    /\.(test|spec)\.[^/]+$/.test(path) || /_tests\.rs$/.test(path) ||
    /(^|\/)test_[^/]+\.py$/.test(path) || /_test\.py$/.test(path);
}

function languagesForPath(path) {
  if (path.startsWith('.github/codeql/') || path === '.github/workflows/codeql.yml' ||
      /^\.github\/scripts\/(codeql-filter|version-only)\.js$/.test(path)) return ALL_LANGUAGES;
  if (/^\.github\/workflows\/.*\.ya?ml$/.test(path) || /(^|\/)action\.ya?ml$/.test(path)) {
    return ['actions'];
  }
  if (/\.rs$/.test(path) || /(^|\/)Cargo\.(toml|lock)$/.test(path) ||
      /(^|\/)\.cargo\//.test(path) || /(^|\/)rust-toolchain(\.toml)?$/.test(path)) return ['rust'];
  if (/^src-tauri\/(tauri\.conf\.(json|json5)|Tauri\.toml|capabilities\/)/.test(path)) {
    return ['javascript-typescript', 'rust'];
  }
  if (/\.pyi?$/.test(path) ||
      /(^|\/)(requirements[^/]*\.txt|pyproject\.toml|Pipfile(\.lock)?|poetry\.lock|uv\.lock|setup\.cfg)$/.test(path)) {
    return ['python'];
  }
  if (/^\.github\/scripts\/.*\.[cm]?[jt]s$/.test(path)) return ['actions', 'javascript-typescript'];
  if (path.startsWith('src-tauri/ui/') || /\.(js|jsx|mjs|cjs|ts|tsx|mts|cts|svelte|vue|html)$/.test(path) ||
      /(^|\/)(package(-lock)?\.json|pnpm-lock\.yaml|yarn\.lock)$/.test(path)) return ['javascript-typescript'];
  return ALL_LANGUAGES;
}

function selectLanguages(files, expectedCount) {
  // GitHub caps PR file lists; incomplete results must never skip analysis.
  if (files.length === 0 || files.length !== expectedCount) return ALL_LANGUAGES;
  const languages = new Set();
  for (const file of files) {
    if (isWorkspaceVersionOnly([file])) continue;
    for (const path of [file.filename, file.previous_filename]) {
      if (path && !isExcluded(path)) {
        for (const language of languagesForPath(path)) languages.add(language);
      }
    }
  }
  return ALL_LANGUAGES.filter(language => languages.has(language));
}

module.exports = { ALL_LANGUAGES, selectLanguages };

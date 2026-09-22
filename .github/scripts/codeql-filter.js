const { isWorkspaceVersionOnly } = require('./version-only.js');

function isExcluded(path) {
  return /\.(md|rst)$/i.test(path) || path === 'LICENSE' ||
    /(^|\/)(tests|__tests__)\//.test(path) ||
    /\.(test|spec)\.[^/]+$/.test(path) || /_tests\.rs$/.test(path) ||
    /(^|\/)test_[^/]+\.py$/.test(path) || /_test\.py$/.test(path);
}

function shouldAnalyze(files, expectedCount) {
  // GitHub caps PR file lists; incomplete results must never skip analysis.
  if (files.length === 0 || files.length !== expectedCount) return true;
  const relevant = files.filter(file => !isExcluded(file.filename) ||
    (file.previous_filename && !isExcluded(file.previous_filename)));
  return relevant.length > 0 && !isWorkspaceVersionOnly(relevant);
}

module.exports = { shouldAnalyze };

// Cargo.toml is CRLF, so patch lines arrive with a trailing \r.
const VERSION_LINE = /^[+-]\s*version\s*=\s*".*"\s*$/;

/**
 * Whether `sha` changes nothing but the workspace version, plus its first parent.
 * `getCommit` diffs against that parent, which is what "this commit alone" means.
 */
async function inspectVersionBump({ github, context, sha }) {
  const { data } = await github.rest.repos.getCommit({
    owner: context.repo.owner,
    repo: context.repo.repo,
    ref: sha,
  });

  const parent = data.parents?.[0]?.sha ?? null;
  const files = data.files ?? [];
  if (files.length !== 1 || files[0].filename !== 'Cargo.toml') {
    return { isVersionOnly: false, parent };
  }

  const patch = (files[0].patch ?? '').split('\n').map((line) => line.replace(/\r$/, ''));
  const changed = patch.filter(
    (line) => /^[+-]/.test(line) && !/^(\+\+\+|---)/.test(line)
  );

  // A `version = "…"` line also appears under [dependencies.x] tables; requiring
  // the section header in the hunk's context keeps a dependency bump from
  // reading as a release bump.
  const inWorkspacePackage = patch.some((line) => line.trim() === '[workspace.package]');

  return {
    isVersionOnly:
      changed.length > 0 && inWorkspacePackage && changed.every((line) => VERSION_LINE.test(line)),
    parent,
  };
}

module.exports = { inspectVersionBump };

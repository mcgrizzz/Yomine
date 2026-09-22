function isWorkspaceVersionOnly(files) {
  if (files.length !== 1) return false;
  const file = files[0];
  if (file.filename !== 'Cargo.toml' || file.status !== 'modified' ||
      file.additions !== 1 || file.deletions !== 1) return false;

  let section;
  let changed = 0;
  for (const line of (file.patch ?? '').split('\n')) {
    // Each hunk must supply its own section context.
    if (line.startsWith('@@')) section = undefined;
    if (/^ \s*\[/.test(line)) section = line.trim();
    if (/^[+-]/.test(line)) {
      if (section !== '[workspace.package]' || !/^[+-]\s*version\s*=\s*"[^"\r\n]+"\s*$/.test(line)) {
        return false;
      }
      changed++;
    }
  }
  return changed === 2;
}

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

  return {
    isVersionOnly: isWorkspaceVersionOnly(data.files ?? []),
    parent: data.parents?.[0]?.sha ?? null,
  };
}

module.exports = { inspectVersionBump, isWorkspaceVersionOnly };

const { inspectVersionBump } = require('./version-only.js');

// A bump's tree differs from its parent by one string, so the parent's run is
// the meaningful one — and since test.yml skips the suite for version-only
// pushes, the bump's own run proves nothing.
async function meaningfulSha({ github, context, sha }) {
  for (let hop = 0; hop < 5; hop++) {
    const { isVersionOnly, parent } = await inspectVersionBump({ github, context, sha });
    if (!isVersionOnly || !parent) return sha;
    console.log(`${sha} only bumps the version; checking its parent ${parent} instead.`);
    sha = parent;
  }
  return sha;
}

async function ensureTests({ github, context, core }) {
  const sha = await meaningfulSha({ github, context, sha: context.sha });
  console.log(`Checking CI status for commit: ${sha}`);

  // Gate on the whole Tests workflow run rather than individual check runs:
  // job-level skips would need name-matching that breaks whenever test.yml is
  // reorganized.
  const { data } = await github.rest.actions.listWorkflowRuns({
    owner: context.repo.owner,
    repo: context.repo.repo,
    workflow_id: 'test.yml',
    head_sha: sha,
  });

  const run = data.workflow_runs[0];
  if (!run) {
    core.setFailed(
      `No Tests workflow run found for ${sha}. Push to main triggers one; wait for it to appear.`
    );
    return;
  }

  if (run.status !== 'completed') {
    core.setFailed(
      `Tests for ${sha} are still ${run.status}. Please wait for tests to complete.`
    );
    return;
  }

  if (run.conclusion !== 'success') {
    core.setFailed(
      `Tests for ${sha} concluded with: ${run.conclusion}. Please fix failing tests before releasing.`
    );
    return;
  }

  console.log(`✅ Tests passed for ${sha} (${run.html_url})`);
}

module.exports = { ensureTests };

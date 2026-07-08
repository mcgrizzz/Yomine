async function ensureTests({ github, context, core }) {
  const sha = context.sha;
  console.log(`Checking CI status for commit: ${sha}`);

  // Gate on the whole Tests workflow run rather than individual check runs:
  // every job runs on push to main, and job-level skips would otherwise need
  // name-matching that breaks whenever test.yml is reorganized.
  const { data } = await github.rest.actions.listWorkflowRuns({
    owner: context.repo.owner,
    repo: context.repo.repo,
    workflow_id: 'test.yml',
    head_sha: sha,
  });

  const run = data.workflow_runs[0];
  if (!run) {
    core.setFailed(
      'No Tests workflow run found for this commit. Push to main triggers one; wait for it to appear.'
    );
    return;
  }

  if (run.status !== 'completed') {
    core.setFailed(
      `Tests are still ${run.status}. Please wait for tests to complete.`
    );
    return;
  }

  if (run.conclusion !== 'success') {
    core.setFailed(
      `Tests concluded with: ${run.conclusion}. Please fix failing tests before releasing.`
    );
    return;
  }

  console.log(`✅ Tests passed for this commit (${run.html_url})`);
}

module.exports = { ensureTests };

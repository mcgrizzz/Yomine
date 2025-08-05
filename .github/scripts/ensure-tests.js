async function ensureTests({ github, context, core }) {
  const sha = context.sha;
  console.log(`Checking CI status for commit: ${sha}`);

  const { data: checkRuns } = await github.rest.checks.listForRef({
    owner: context.repo.owner,
    repo: context.repo.repo,
    ref: sha,
  });

  const testWorkflow = checkRuns.check_runs.find(
    (run) =>
      run.name === 'test' || (run.name || '').toLowerCase().includes('tests')
  );

  if (!testWorkflow) {
    console.log('⚠️  No test workflow found for this commit');
    console.log('Available check runs:', checkRuns.check_runs.map((r) => r.name));
    core.setFailed(
      'No CI tests found for this commit. Please ensure tests have run first.'
    );
    return;
  }

  if (testWorkflow.status !== 'completed') {
    core.setFailed(
      `Tests are still ${testWorkflow.status}. Please wait for tests to complete.`
    );
    return;
  }

  if (testWorkflow.conclusion !== 'success') {
    core.setFailed(
      `Tests failed with status: ${testWorkflow.conclusion}. Please fix failing tests before releasing.`
    );
    return;
  }

  console.log('✅ Tests passed for this commit');
}

module.exports = { ensureTests };
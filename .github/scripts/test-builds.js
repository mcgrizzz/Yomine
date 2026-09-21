const platforms = ['windows', 'linux', 'macos'];
const buildLabels = [...platforms, 'all'].map(p => `test-build:${p}`);

async function requestBuild({ github, context, core }) {
  const number = Number(process.env.PR_NUMBER);
  const label = `test-build:${process.env.PLATFORM}`;
  if (!Number.isSafeInteger(number) || number <= 0) throw new Error('Enter a valid PR number.');
  if (!buildLabels.includes(label)) throw new Error('Select windows, linux, macos, or all.');
  const { data: pr } = await github.rest.pulls.get({ ...context.repo, pull_number: number });
  if (pr.state !== 'open' || !pr.head.repo) throw new Error('Select an open PR with an available source repository.');
  if (pr.mergeable === false) throw new Error('Resolve the PR merge conflicts before requesting a build.');
  try {
    await github.rest.issues.createLabel({
      ...context.repo, name: label, color: '1d76db', description: 'Opt-in test installer build',
    });
  } catch (error) {
    if (error.status !== 422) throw error;
  }
  for (const old of pr.labels.filter(l => buildLabels.includes(l.name))) {
    await github.rest.issues.removeLabel({ ...context.repo, issue_number: number, name: old.name });
  }
  await github.rest.issues.addLabels({ ...context.repo, issue_number: number, labels: [label] });
  await core.summary.addRaw(`Requested ${process.env.PLATFORM} installers for [PR #${number}](${pr.html_url}). Download links will be posted when the PR build finishes.`).write();
}

async function share({ github, context, core }) {
  const run = context.payload.workflow_run;
  if (run.event !== 'pull_request' || run.path !== '.github/workflows/pr-test-build.yml' || run.conclusion === 'cancelled') return;
  const candidates = await github.paginate(github.rest.repos.listPullRequestsAssociatedWithCommit, {
    ...context.repo, commit_sha: run.head_sha, per_page: 100,
  });
  const prs = candidates.filter(pr => pr.state === 'open'
    && pr.head.sha === run.head_sha && pr.head.ref === run.head_branch
    && pr.head.repo?.id === run.head_repository.id
    && pr.base.repo.full_name === `${context.repo.owner}/${context.repo.repo}`);
  if (prs.length !== 1) {
    core.notice('No unique, current PR matches the build; leaving downloads on the workflow run.');
    return;
  }
  const number = prs[0].number;
  const sha = run.head_sha;
  const artifacts = await github.paginate(github.rest.actions.listWorkflowRunArtifacts, {
    ...context.repo, run_id: run.id, per_page: 100,
  });
  const prefix = `pr-${number}-${sha}-`;
  const downloads = artifacts.filter(a => !a.expired && platforms.some(p => a.name === prefix + p));
  if (!downloads.length) {
    core.notice('No test installers were produced; no download comments will be posted.');
    return;
  }
  const { repository: { pullRequest: pr } } = await github.graphql(`
    query($owner: String!, $repo: String!, $number: Int!) {
      repository(owner: $owner, name: $repo) {
        pullRequest(number: $number) {
          state headRefOid
          closingIssuesReferences(first: 100) {
            nodes { number repository { nameWithOwner } }
          }
        }
      }
    }`, { ...context.repo, number });
  if (pr.state !== 'OPEN' || pr.headRefOid !== sha) return;
  const repoName = `${context.repo.owner}/${context.repo.repo}`;
  const issues = pr.closingIssuesReferences.nodes
    .filter(issue => issue.repository.nameWithOwner === repoName)
    .map(issue => issue.number);
  const repoUrl = `${context.serverUrl}/${repoName}`;
  const runUrl = `${repoUrl}/actions/runs/${run.id}`;
  const marker = `<!-- yomine-test-build:pr-${number} -->`;
  const body = [
    marker,
    `Test installers for [PR #${number}](${repoUrl}/pull/${number}), commit [${sha.slice(0, 7)}](${repoUrl}/commit/${sha}).`,
    '',
    ...downloads.map(a => `- [${a.name.slice(prefix.length)}](${runUrl}/artifacts/${a.id}) — expires ${a.expires_at.slice(0, 10)}`),
    '',
    'Sign into GitHub to download. Extract the ZIP and run the installer inside.',
    'Please report whether this build fixes the issue.',
    '',
    run.conclusion === 'success'
      ? `[Build details](${runUrl})`
      : `Some platforms failed to build. [Build details](${runUrl})`,
  ].join('\n');
  await core.summary.addRaw(body).write();
  for (const issue_number of new Set([number, ...issues])) {
    const comments = await github.paginate(github.rest.issues.listComments, {
      ...context.repo, issue_number, per_page: 100,
    });
    const existing = comments.find(c => c.user.login === 'github-actions[bot]' && c.body.startsWith(marker));
    if (existing) {
      await github.rest.issues.updateComment({ ...context.repo, comment_id: existing.id, body });
    } else {
      await github.rest.issues.createComment({ ...context.repo, issue_number, body });
    }
  }
}

module.exports = { requestBuild, share };

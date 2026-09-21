const platforms = [
  { platform: 'windows', runner: 'windows-latest', args: '' },
  { platform: 'linux', runner: 'ubuntu-22.04', args: '' },
  { platform: 'macos', runner: 'macos-latest', args: '--target universal-apple-darwin' },
];

async function prepare({ github, context, core }) {
  const number = Number(process.env.PR_NUMBER);
  if (!Number.isSafeInteger(number) || number <= 0) throw new Error('Enter a valid PR number.');
  const matrix = platforms.filter(p => process.env.PLATFORM === 'all' || p.platform === process.env.PLATFORM);
  if (!matrix.length) throw new Error('Select windows, linux, macos, or all.');
  const { repository: { pullRequest: pr } } = await github.graphql(`
    query($owner: String!, $repo: String!, $number: Int!) {
      repository(owner: $owner, name: $repo) {
        pullRequest(number: $number) {
          state headRefOid
          headRepository { nameWithOwner }
          closingIssuesReferences(first: 100) {
            nodes { number repository { nameWithOwner } }
          }
        }
      }
    }`, { ...context.repo, number });
  if (!pr || pr.state !== 'OPEN' || !pr.headRepository) throw new Error('Select an open PR with an available source repository.');
  const repository = `${context.repo.owner}/${context.repo.repo}`;
  const issues = pr.closingIssuesReferences.nodes
    .filter(issue => issue.repository.nameWithOwner === repository)
    .map(issue => issue.number);
  core.setOutput('sha', pr.headRefOid);
  core.setOutput('repository', pr.headRepository.nameWithOwner);
  core.setOutput('issues', issues);
  core.setOutput('matrix', { include: matrix });
}

async function share({ github, context, core }) {
  const number = Number(process.env.PR_NUMBER);
  const sha = process.env.BUILD_SHA;
  const { data: pr } = await github.rest.pulls.get({ ...context.repo, pull_number: number });
  if (pr.state !== 'open' || pr.head.sha !== sha) {
    core.notice('PR changed or closed during the build; leaving downloads on the workflow run.');
    return;
  }
  const artifacts = await github.paginate(github.rest.actions.listWorkflowRunArtifacts, {
    ...context.repo, run_id: context.runId, per_page: 100,
  });
  const prefix = `pr-${number}-${sha}-`;
  const downloads = artifacts.filter(a => !a.expired && a.name.startsWith(prefix));
  if (!downloads.length) throw new Error('No test installers were produced. Check the build jobs.');
  const repoUrl = `${context.serverUrl}/${context.repo.owner}/${context.repo.repo}`;
  const runUrl = `${repoUrl}/actions/runs/${context.runId}`;
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
    process.env.BUILD_RESULT === 'success'
      ? `[Build details](${runUrl})`
      : `Some platforms failed to build. [Build details](${runUrl})`,
  ].join('\n');
  await core.summary.addRaw(body).write();
  const targets = new Set([number, ...JSON.parse(process.env.LINKED_ISSUES)]);
  for (const issue_number of targets) {
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

module.exports = { prepare, share };

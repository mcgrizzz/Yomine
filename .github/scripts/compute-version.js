function parseCargoVersion(cargoToml) {
  // The version lives in [workspace.package] (both crates inherit it);
  // fall back to a quoted [package] version for older layouts.
  const lines = cargoToml.split(/\r?\n/);
  let section = '';
  let packageVersion = null;
  for (const raw of lines) {
    const line = raw.trim();
    const sec = line.match(/^\[(.+)\]$/);
    if (sec) {
      section = sec[1];
      continue;
    }
    const m = line.match(/^version\s*=\s*"(.*?)"/);
    if (m) {
      if (section === 'workspace.package') return m[1];
      if (section === 'package') packageVersion = m[1];
    }
  }
  if (packageVersion) return packageVersion;
  throw new Error(
    'Could not find version in Cargo.toml ([workspace.package] or [package])'
  );
}

function toTag(version) {
  return `v${version}`;
}

function parseExistingBetaNumber(tags, baseVersion) {
  // Prioritize short form but maintain backward compatibility
  const reShort = new RegExp(`^v${escapeRegExp(baseVersion)}b(\\d+)$`);
  const reLong = new RegExp(`^v${escapeRegExp(baseVersion)}-(?:beta)\\.(\\d+)$`);

  let max = 0;
  for (const t of tags) {
    let m = t.match(reShort);
    if (m) {
      const n = parseInt(m[1], 10);
      if (!Number.isNaN(n)) max = Math.max(max, n);
      continue;
    }

    m = t.match(reLong);
    if (m) {
      const n = parseInt(m[1], 10);
      if (!Number.isNaN(n)) max = Math.max(max, n);
    }
  }
  return max;
}

function escapeRegExp(s) {
  return s.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
}

async function computeVersion({ github, context, cargoToml, releaseType }) {
  const baseVersion = parseCargoVersion(cargoToml); // e.g. 1.2.3
  const owner = context.repo.owner;
  const repo = context.repo.repo;

  const tags = [];
  let page = 1;
  while (true) {
    const { data } = await github.rest.repos.listTags({
      owner,
      repo,
      per_page: 100,
      page,
    });
    if (!data || data.length === 0) break;
    for (const t of data) tags.push(t.name);
    page += 1;
  }

  if (releaseType === 'stable') {
    const tag = toTag(baseVersion); // v1.2.3
    if (tags.includes(tag)) {
      throw new Error(`Version ${tag} already exists`);
    }
    return {
      version: tag,
      isPrerelease: false,
      baseVersion,
    };
  }

  const currentMax = parseExistingBetaNumber(tags, baseVersion);
  const next = currentMax + 1;
  const tag = `v${baseVersion}b${next}`; 
  return {
    version: tag,
    isPrerelease: true,
    baseVersion,
  };
}

function buildReleaseName({ version, baseVersion, isPrerelease }) {
  if (isPrerelease) {
    const mShort = version.match(/^v(\d+\.\d+\.\d+)b(\d+)$/);
    if (mShort) {
      const betaNum = mShort[2];
      return `${baseVersion} Beta ${betaNum}`;
    }
    const mLong = version.match(/^v(\d+\.\d+\.\d+)-(beta)\.(\d+)$/);
    if (mLong) {
      const betaType = mLong[2];
      const betaNum = mLong[3];
      return `${baseVersion} ${betaType.charAt(0).toUpperCase()}${betaType.slice(1)} ${betaNum}`;
    }
    return version;
  }
  
  return baseVersion;
}

module.exports = { computeVersion, buildReleaseName };
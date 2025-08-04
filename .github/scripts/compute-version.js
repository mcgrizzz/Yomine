function parseCargoVersion(cargoToml) {
  const afterPackage = cargoToml.split(/\r?\n/);
  let inPackage = false;
  for (let i = 0; i < afterPackage.length; i++) {
    const line = afterPackage[i].trim();
    if (line === '[package]') {
      inPackage = true;
      continue;
    }
    if (inPackage) {
      if (line.startsWith('[') && line !== '[package]') break;
      const m = line.match(/^version\s*=\s*"(.*?)"/);
      if (m) return m[1];
    }
  }
  throw new Error('Could not find version in Cargo.toml [package] section');
}

function toTag(version) {
  return `v${version}`;
}

function parseExistingBetaNumber(tags, baseVersion) {
  const reHyphen = new RegExp(`^v${escapeRegExp(baseVersion)}-(?:beta)\\.(\\d+)$`);
  const reShort = new RegExp(`^v${escapeRegExp(baseVersion)}b(\\d+)$`);

  let max = 0;
  for (const t of tags) {
    let m = t.match(reHyphen);
    if (m) {
      const n = parseInt(m[1], 10);
      if (!Number.isNaN(n)) max = Math.max(max, n);
      continue;
    }
    m = t.match(reShort);
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
  const tag = `v${baseVersion}-beta.${next}`;
  return {
    version: tag,
    isPrerelease: true,
    baseVersion,
  };
}

function buildReleaseName({ version, baseVersion, isPrerelease }) {
  if (isPrerelease) {
    const m = version.match(/^v(\d+\.\d+\.\d+)-(beta)\.(\d+)$/);
    if (m) {
      const betaType = m[2];
      const betaNum = m[3];
      return `${baseVersion} ${betaType.charAt(0).toUpperCase()}${betaType.slice(
        1
      )} ${betaNum}`;
    }
    return version;
  }
  
  return baseVersion;
}

module.exports = { computeVersion, buildReleaseName };
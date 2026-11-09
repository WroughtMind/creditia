// Only advertise channels that exist for this source version. A lookup failure
// aborts the build instead of turning a network error into a false availability claim.
import { readFileSync } from 'node:fs';
import { apps, version } from '../packaging/release.mjs';
const channel = JSON.parse(readFileSync(new URL('../packaging/desktop-channel.json', import.meta.url)));
if (!/^gpui-v[0-9]+\.[0-9]+\.[0-9]+-[a-zA-Z0-9.-]+$/.test(channel.tag) || !/^[a-f0-9]{40}$/.test(channel.candidate_sha)) throw Error('Invalid verified desktop channel');
const REPO = 'https://github.com/WroughtMind/creditia';
export async function downloads(request = fetch) {
  async function get(url, github = false) {
    const headers = github && process.env.GH_TOKEN ? { Authorization: `Bearer ${process.env.GH_TOKEN}` } : {};
    const r = await request(url, { headers, signal: AbortSignal.timeout(30000) });
    if (r.status === 404) return null;
    if (!r.ok) throw Error(`Availability lookup failed: ${r.status} ${url}`);
    return r;
  }
  const [releaseResponse, desktopResponse, npmResponse, caskResponse] = await Promise.all([
    get(`https://api.github.com/repos/WroughtMind/creditia/releases/tags/v${version}`, true),
    get(`https://api.github.com/repos/WroughtMind/creditia/releases/tags/${channel.tag}`, true),
    get(`https://registry.npmjs.org/creditia/${version}`),
    get('https://raw.githubusercontent.com/taekchef/homebrew-tap/HEAD/Casks/creditia.rb'),
  ]);
  const release = releaseResponse ? await releaseResponse.json() : null;
  const published = release && !release.draft && !release.prerelease && release.tag_name === `v${version}`;
  const desktop = desktopResponse ? await desktopResponse.json() : null;
  const publishedCandidate = desktop && !desktop.draft && desktop.prerelease && desktop.tag_name === channel.tag &&
    desktop.target_commitish === channel.candidate_sha;
  const stableNames = new Set(published ? release.assets.map(asset => asset.name) : []);
  const stableDesktop = published && apps.some(app => stableNames.has(app.file) && stableNames.has(`${app.file}.sha256`));
  const source = stableDesktop ? release : publishedCandidate ? desktop : null;
  const assets = new Map(source ? source.assets.map(a => [a.name, a.browser_download_url]) : []);
  const checksumFor = name => stableDesktop ? assets.get(`${name}.sha256`) : assets.get('SHA256SUMS');
  const available = apps.filter(a => assets.has(a.file) && checksumFor(a.file)).map(a => ({
    ...a, url: assets.get(a.file), checksum: checksumFor(a.file),
  }));
  const npm = npmResponse ? await npmResponse.json() : null;
  const cask = caskResponse ? await caskResponse.text() : '';
  const brew = Boolean(stableDesktop) && available.filter(a => a.os === 'mac' && a.bundle === 'dmg').length === 2
    && cask.includes(`version "${version}"`)
    && cask.includes('WroughtMind/creditia/releases/download/')
    && cask.includes('creditia-gpui-#{arch}-apple-darwin.dmg')
    && /sha256 arm: "(?!0{64})[a-f0-9]{64}", intel: "(?!0{64})[a-f0-9]{64}"/.test(cask);
  return { catalog: apps, apps: available, desktopTag: source?.tag_name, npm: npm?.name === 'creditia' && npm?.version === version, brew, releases: `${REPO}/releases` };
}

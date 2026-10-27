// Only advertise channels that exist for this source version. A lookup failure
// aborts the build instead of turning a network error into a false availability claim.
import { apps, version } from '../packaging/release.mjs';
const REPO = 'https://github.com/WroughtMind/creditia';
export async function downloads(request = fetch) {
  async function get(url, github = false) {
    const headers = github && process.env.GH_TOKEN ? { Authorization: `Bearer ${process.env.GH_TOKEN}` } : {};
    const r = await request(url, { headers, signal: AbortSignal.timeout(30000) });
    if (r.status === 404) return null;
    if (!r.ok) throw Error(`Availability lookup failed: ${r.status} ${url}`);
    return r;
  }
  const [releaseResponse, npmResponse, caskResponse] = await Promise.all([
    get(`https://api.github.com/repos/WroughtMind/creditia/releases/tags/v${version}`, true),
    get(`https://registry.npmjs.org/creditia/${version}`),
    get('https://raw.githubusercontent.com/taekchef/homebrew-tap/HEAD/Casks/creditia.rb'),
  ]);
  const release = releaseResponse ? await releaseResponse.json() : null;
  const published = release && !release.draft && !release.prerelease && release.tag_name === `v${version}`;
  const assets = new Map(published ? release.assets.map(a => [a.name, a.browser_download_url]) : []);
  const available = apps.filter(a => assets.has(a.file) && assets.has(`${a.file}.sha256`)).map(a => ({ ...a, url: assets.get(a.file), checksum: assets.get(`${a.file}.sha256`) }));
  const npm = npmResponse ? await npmResponse.json() : null;
  const cask = caskResponse ? await caskResponse.text() : '';
  const brew = available.filter(a => a.os === 'mac').length === 2
    && cask.includes(`version "${version}"`)
    && cask.includes('WroughtMind/creditia/releases/download/')
    && /sha256 arm: "(?!0{64})[a-f0-9]{64}", intel: "(?!0{64})[a-f0-9]{64}"/.test(cask);
  return { catalog: apps, apps: available, npm: npm?.name === 'creditia' && npm?.version === version, brew, releases: `${REPO}/releases` };
}

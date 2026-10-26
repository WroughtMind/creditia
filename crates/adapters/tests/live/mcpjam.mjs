// Account-free, real-executor acceptance. The model alone is scripted.
// node mcpjam.mjs <MCPJam checkout> <npm dependencies> <Codex vendor> <new output dir> <creditia-core> [attended|unattended]
import assert from 'node:assert/strict';
import { spawn, execFileSync } from 'node:child_process';
import { createHash, randomUUID } from 'node:crypto';
import { mkdirSync, writeFileSync, readFileSync, existsSync, symlinkSync, readlinkSync, readdirSync, copyFileSync } from 'node:fs';
import { join, resolve } from 'node:path';
import { pathToFileURL } from 'node:url';

const [sourceArg, depsArg, vendorArg, outputArg, creditiaArg, mode = 'attended'] = process.argv.slice(2);
assert(sourceArg && depsArg && vendorArg && outputArg && creditiaArg, 'Five paths required; see file header.');
assert(['attended', 'unattended'].includes(mode));
const [source, deps, vendor, root, creditia] = [sourceArg, depsArg, vendorArg, outputArg, creditiaArg].map(p => resolve(p));
assert(!existsSync(root), 'Use a fresh output directory; preserve prior evidence.');
const unattended = mode === 'unattended';
const upstream = join(source, 'mcpjam-inspector/server/utils/harness');
const load = path => import(pathToFileURL(path).href);
const { build } = await load(join(deps, 'node_modules/esbuild/lib/main.js'));
const { WebSocket } = await load(join(deps, 'node_modules/ws/wrapper.mjs'));
const { createFakeResponsesServer } = await load(join(source, '.spike-codex-appserver/probe/fake-responses-server.mjs'));
const layer = join(root, 'layer');
const home = join(root, 'home');
const repo = join(root, 'repo');
const state = join(home, '.mcpjam/harness-local-sessions/acceptance');
const work = join(state, 'work');
const project = join(work, 'project');
const data = join(work, '.agent-runs/native');
for (const path of [layer, repo, work, join(state, 'home'), join(state, 'tmp')]) mkdirSync(path, { recursive: true, mode: 0o700 });
symlinkSync(repo, project);
const git = args => execFileSync('git', args, { cwd: repo, stdio: 'pipe' });
git(['init', '-q', '-b', 'main']);
const commit = message => git(['-c', 'user.name=Acceptance', '-c', 'user.email=acceptance@example.invalid', 'commit', '--allow-empty', '-qm', message]);
commit('human baseline');
const version = execFileSync(process.execPath, [join(vendor, 'node_modules/@openai/codex/bin/codex.js'), '--version'], { encoding: 'utf8' }).trim();
const digest = path => createHash('sha256').update(readFileSync(path)).digest('hex');
const sourceFiles = ['local/command-translation.ts', 'local/confine.ts', 'local/layer/launcher.mjs', 'codex-appserver/bridge/index.ts', 'codex-appserver/shared/sandbox-policy.ts'];
writeFileSync(join(root, 'sources.json'), JSON.stringify({ version, node: process.version, mode,
  commit: existsSync(join(source, '.git')) ? execFileSync('git', ['-C', source, 'rev-parse', 'HEAD'], { encoding: 'utf8' }).trim() : null,
  sha256: Object.fromEntries(sourceFiles.map(p => [p, digest(join(upstream, p))])) }, null, 2));

// Same local-layer build shape as upstream: bundled dependencies, optional
// native ws accelerators disabled, and the official loopback-only launcher.
const buildOptions = { bundle: true, platform: 'node', format: 'esm', target: 'node22', nodePaths: [join(deps, 'node_modules')] };
const stubOptionalNatives = { name: 'optional-ws-natives', setup(build) {
  build.onResolve({ filter: /^(bufferutil|utf-8-validate)$/ }, args => ({ path: args.path, namespace: 'optional-native' }));
  build.onLoad({ filter: /.*/, namespace: 'optional-native' }, () => ({ contents: 'throw new Error("Optional native dependency excluded from local layer");', loader: 'js' }));
} };
for (const name of ['index', 'host-tools-mcp']) await build({ ...buildOptions,
  entryPoints: [join(upstream, `codex-appserver/bridge/${name}.ts`)],
  outfile: join(layer, name === 'index' ? 'bridge.mjs' : 'host-tools-mcp.mjs'),
  banner: { js: 'import {createRequire} from "node:module"; const require=createRequire(import.meta.url);' },
  footer: { js: name === 'index' ? 'main();' : 'startHostToolMcpServer();' }, plugins: [stubOptionalNatives],
});
copyFileSync(join(upstream, 'local/layer/launcher.mjs'), join(layer, 'launcher.mjs'));
await build({ ...buildOptions, stdin: { contents: [
  `export {translateAdapterCommand} from ${JSON.stringify(join(upstream, 'local/command-translation.ts'))};`,
  `export {confinePath} from ${JSON.stringify(join(upstream, 'local/confine.ts'))};`,
  `export {LOCAL_UNATTENDED_SANDBOX_POLICY} from ${JSON.stringify(join(upstream, 'codex-appserver/shared/sandbox-policy.ts'))};`,
].join('\n'), resolveDir: source, loader: 'ts' }, outfile: join(root, 'launch.mjs') });
const { translateAdapterCommand, confinePath, LOCAL_UNATTENDED_SANDBOX_POLICY } = await load(join(root, 'launch.mjs'));
const quote = text => "'" + text.replaceAll("'", "'\\''") + "'";
const boot = join(work, '.harness-bootstrap/codex');
const command = `node ${quote(join(boot, 'bridge.mjs'))} --workdir ${quote(project)} --bridge-state-dir ${quote(join(state, 'bridge'))} --session-data-dir ${quote(data)} --bootstrap-dir ${quote(boot)}`;
const ctx = { harnessId: 'codex', adapterBootstrapDir: boot, managedBundleRoot: vendor,
  layerRoot: layer, layerFiles: ['bridge.mjs', 'host-tools-mcp.mjs', 'launcher.mjs'],
  adapterBootstrapFiles: ['bridge.mjs', 'host-tools-mcp.mjs'], bootstrapOverlayDir: join(state, 'bootstrap'),
  bridgeLauncherPath: join(layer, 'launcher.mjs'), launcherVendorRoot: vendor,
  nodeExecutable: process.execPath, sessionRoot: work, syntheticHome: join(state, 'home'),
  confine: path => confinePath(path, { roots: [state, repo] }),
  confineToSessionState: path => confinePath(path, { roots: [state] }),
};
const launch = await translateAdapterCommand({ command }, ctx);
assert.equal(launch.kind, 'exec');
assert.equal(readlinkSync(project), repo);
assert.equal(launch.args[launch.args.indexOf('--workdir') + 1], repo);
// A link outside the grant must fail, not silently add a writable root.
const foreign = join(root, 'outside');
mkdirSync(foreign);
symlinkSync(foreign, join(work, 'foreign'));
await assert.rejects(() => translateAdapterCommand({ command: command.replace(quote(project), quote(join(work, 'foreign'))) }, ctx), /outside every directory/);
writeFileSync(join(root, 'launch.json'), JSON.stringify({ requestedWorkdir: project, launch }, null, 2));

const content = `creditia-mcpjam-native\nexecutor=${version}\nmodel=scripted-test\n`;
const patch = (name, text) => ({ customToolCalls: [{ name: 'apply_patch', input: `*** Begin Patch\n*** Add File: ${name}\n${text.trimEnd().split('\n').map(l => '+' + l).join('\n')}\n*** End Patch` }] });
// macOS zsh defaults TMPPREFIX to /tmp/zsh even with a private TMPDIR.
// Bash honours the permitted TMPDIR for heredocs; the OS policy is unchanged.
const shell = cmd => ({ functionCalls: [{ name: 'exec_command', arguments: { cmd, yield_time_ms: 1,
  ...(unattended ? { shell: '/bin/bash', login: false } : {}) } }] });
let fake;
const poll = { get functionCalls() {
  const output = [...fake.requests.at(-1).body.input].reverse().find(x => x.type === 'function_call_output' && /Process running with session ID \d+/.test(String(x.output)));
  assert(output, 'Poll must use a process handle returned by the actual executor.');
  return [{ name: 'write_stdin', arguments: { session_id: Number(String(output.output).match(/Process running with session ID (\d+)/)[1]), chars: '', yield_time_ms: 1000 } }];
} };
const script = [];
fake = createFakeResponsesServer({ script, logPath: join(root, 'model-requests.jsonl'), strictToolNames: true });
const baseUrl = await fake.listen();
const token = randomUUID();
const all = [];
let bridge, socket;
try {
  const port = await new Promise((resolve, reject) => {
    const timer = setTimeout(() => reject(new Error('Bridge startup timeout')), 45000);
    bridge = spawn(launch.executable, launch.args, { cwd: launch.workingDirectory, detached: true,
      env: { PATH: process.env.PATH, HOME: join(state, 'home'), TMPDIR: join(state, 'tmp'), BRIDGE_CHANNEL_TOKEN: token, BRIDGE_WS_PORT: '0',
        CODEX_API_KEY: 'scripted-test-only', OPENAI_BASE_URL: baseUrl + '/v1', MCPJAM_LOCAL_CONTROL_ROOT: join(home, '.mcpjam/harness-local') },
      stdio: ['pipe', 'pipe', 'pipe'],
    });
    let stdout = '', stderr = '';
    bridge.stderr.on('data', bytes => { stderr += bytes; writeFileSync(join(root, 'bridge-stderr.log'), stderr); });
    bridge.stdout.on('data', bytes => {
      stdout += bytes; writeFileSync(join(root, 'bridge-stdout.log'), stdout);
      for (const line of stdout.split('\n')) {
        try { const f = JSON.parse(line); if (f.type === 'bridge-ready' && f.port) { clearTimeout(timer); resolve(f.port); } } catch {}
      }
    });
    bridge.once('error', error => { clearTimeout(timer); reject(error); });
    bridge.once('exit', code => { clearTimeout(timer); reject(new Error('Bridge exited ' + code)); });
  });
  socket = new WebSocket(`ws://127.0.0.1:${port}/?agent_bridge_token=${token}`);
  await new Promise((resolve, reject) => { socket.once('open', resolve); socket.once('error', reject); });
  async function turn(label, steps, approve = true) {
    script.push(...steps, { text: 'Scripted model turn finished.' });
    const frames = [];
    const result = new Promise((resolve, reject) => {
      const timer = setTimeout(() => reject(new Error(label + ' timed out')), 45000);
      const listener = raw => {
        const f = JSON.parse(String(raw)); frames.push(f);
        if (f.type === 'tool-approval-request') socket.send(JSON.stringify({ type: 'tool-approval-response', approvalId: f.approvalId, approved: approve }));
        if (f.type === 'finish' || f.type === 'error') { clearTimeout(timer); socket.off('message', listener); resolve(); }
      };
      socket.on('message', listener);
    });
    socket.send(JSON.stringify({ type: 'start', prompt: label, tools: [], model: 'gpt-5.5',
      permissionMode: unattended ? 'allow-all' : 'allow-reads', ...(unattended ? { sandboxPolicy: LOCAL_UNATTENDED_SANDBOX_POLICY } : {}) }));
    await result;
    all.push({ label, frames }); writeFileSync(join(root, 'bridge-frames.json'), JSON.stringify(all, null, 2));
    assert(!frames.some(f => f.type === 'error'), JSON.stringify(frames.filter(f => f.type === 'error')));
    if (unattended) assert(!frames.some(f => f.type === 'tool-approval-request'), 'Unattended mode must not ask approval.');
    console.log('Completed:', label);
    return frames;
  }
  const good = await turn('Approved relative write', [patch('verified.txt', content)]);
  assert.equal(readFileSync(join(repo, 'verified.txt'), 'utf8'), content);
  if (!unattended) {
    assert(good.some(f => f.type === 'tool-approval-request'));
    const denied = await turn('Declined write', [patch('denied.txt', 'must not exist\n')], false);
    assert(!existsSync(join(repo, 'denied.txt')));
    assert(denied.some(f => f.type === 'tool-result' && f.isError));
  }
  await turn('Invalid edit', [{ customToolCalls: [{ name: 'apply_patch', input: '*** Begin Patch\n*** Update File: verified.txt\n@@\n-missing line\n+must not count\n*** End Patch' }] }]);
  assert.equal(readFileSync(join(repo, 'verified.txt'), 'utf8'), content);
  assert(JSON.stringify(fake.requests.at(-1).body.input).includes('apply_patch verification failed'));
  const readback = await turn('Native readback', [shell('cat verified.txt')]);
  assert(JSON.stringify(readback).includes(content.split('\n')[0]));
  await turn('Background write and final poll', [shell("sleep 0.5; cat > async.txt <<'EOF'\nasync native execution\nfinished through write_stdin\nEOF"), poll]);
  assert.equal(readFileSync(join(repo, 'async.txt'), 'utf8'), 'async native execution\nfinished through write_stdin\n');
  assert(JSON.stringify(fake.requests.at(-1).body.input).includes('Process exited with code 0'));
  await turn('Failed background write', [shell("sleep 0.5; cat > failed-async.txt <<'EOF'\nfailed async must not count\nEOF\nexit 9"), poll]);
  assert.equal(readFileSync(join(repo, 'failed-async.txt'), 'utf8'), 'failed async must not count\n');
  assert(JSON.stringify(fake.requests.at(-1).body.input).includes('Process exited with code 9'));
  if (unattended) {
    await turn('Outside write blocked', [shell(`printf blocked > ${quote(join(foreign, 'blocked.txt'))}`)]);
    assert(!existsSync(join(foreign, 'blocked.txt')));
    const refusal = [...fake.requests.at(-1).body.input].reverse().find(x => x.type === 'function_call_output');
    assert.match(String(refusal?.output), /Process exited with code 1[\s\S]*Operation not permitted/);
    await turn('Private temporary directory allowed', [shell('printf private-temp > "$TMPDIR/probe.txt"; cat "$TMPDIR/probe.txt"')]);
    assert.equal(readFileSync(join(state, 'tmp/probe.txt'), 'utf8'), 'private-temp');
    const network = `const s=require('node:net').connect(${new URL(baseUrl).port},'127.0.0.1'); s.on('connect',()=>process.exit(0)); s.on('error',e=>{console.log(e.code);process.exit(7)});setTimeout(()=>process.exit(8),2000);`;
    const frames = await turn('Sandbox command network blocked', [shell(`${quote(process.execPath)} -e ${quote(network)}`)]);
    assert(JSON.stringify(frames).includes('EPERM'), 'Require an actual sandbox denial, not a timeout.');
  }
  await turn('Unfinished background write', [shell("cat > pending-async.txt <<'EOF'\npending async must not count\nEOF\nsleep 30")]);
  assert.equal(readFileSync(join(repo, 'pending-async.txt'), 'utf8'), 'pending async must not count\n');
  git(['add', 'verified.txt', 'async.txt', 'failed-async.txt', 'pending-async.txt']);
  commit('save actual executor output');
} finally {
  socket?.terminate();
  if (bridge?.pid && bridge.exitCode === null) {
    const stopped = new Promise(resolve => bridge.once('exit', resolve));
    try { process.kill(-bridge.pid, 'SIGTERM'); } catch {}
    await stopped;
    // Bridge exit precedes the native child's final history flush. Establish
    // that the owned process group is gone before taking read-only snapshots.
    const deadline = Date.now() + 5000;
    for (;;) {
      try { process.kill(-bridge.pid, 0); } catch (error) {
        if (error.code === 'ESRCH') break;
        throw error;
      }
      assert(Date.now() < deadline, 'Owned executor group did not stop.');
      await new Promise(resolve => setTimeout(resolve, 50));
    }
  }
  await fake.close();
}

const nativeHome = join(data, 'codex-home');
const nativeFiles = readdirSync(nativeHome, { recursive: true }).filter(p => p.endsWith('.jsonl') || p === 'state_5.sqlite').map(p => join(nativeHome, p));
assert(nativeFiles.some(p => p.endsWith('.jsonl')));
const snapshots = () => Object.fromEntries(nativeFiles.map(p => [p, digest(p)]));
const before = snapshots();
const runs = [];
for (let i = 0; i < 2; i++) {
  execFileSync(creditia, ['analyze', '--repo', repo, '--json'], { env: { PATH: process.env.PATH, HOME: home, CREDITIA_HOME: join(root, 'library'), CODEX_HOME: join(root, 'empty-codex'), CREDITIA_HOOK_TIMEOUT: '0' } });
  const credits = JSON.parse(readFileSync(join(repo, '.creditia/credits.json'), 'utf8'));
  const verified = credits.breakdown.reduce((sum, b) => sum + (b.byEvidence.verified?.linesAdded ?? 0), 0);
  assert.equal(verified, 5, 'Three patch lines and two completed background lines; failed/pending files excluded.');
  runs.push(credits.breakdown);
}
assert.deepEqual(runs[0], runs[1]);
assert.deepEqual(snapshots(), before, 'Native history must remain unchanged.');
writeFileSync(join(root, 'execution-result.json'), JSON.stringify({ version, mode, requestedWorkdir: project, effectiveWorkdir: repo, verifiedLines: 5,
  scriptedModel: true, turns: all.map(t => t.label), runs, unchangedNativeFiles: before }, null, 2));
console.log('PASS: default linked entry, real executor, native history, five verified lines, repeat scan unchanged.');

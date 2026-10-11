import { defineConfig } from 'vite';
import path from 'node:path';
import { spawn } from 'node:child_process';
const ROOT = path.resolve(import.meta.dirname, '../..');
function localCore() {
  return {
    name: 'creditia-local-core', apply: 'serve' as const,
    configureServer(server: any) {
      server.middlewares.use('/__dev', async (req: any, res: any) => {
        const origin = req.headers.origin;
        let local = !origin;
        try { if (origin) local = new URL(origin).host === req.headers.host; } catch { local = false; }
        if (!local) { res.statusCode = 403; res.end(JSON.stringify({ error: '仅接受本地同源请求' })); return; }
        if (req.method !== 'POST') { res.statusCode = 405; res.end('{}'); return; }
        const command = String(req.url).slice(1).split('?')[0];
        if (!['projects','library_credits','credits','config_of','doctor','analyze_project','set_people','link_path'].includes(command)) { res.statusCode = 404; res.end('{}'); return; }
        const chunks = []; for await (const chunk of req) chunks.push(chunk);
        const body = Buffer.concat(chunks).toString();
        try { JSON.parse(body); } catch { res.statusCode = 400; res.end(JSON.stringify({ error: '请求格式无效' })); return; }
        const binary = process.env.CREDITIA_CORE ?? path.join(ROOT, 'target/debug', process.platform === 'win32' ? 'creditia-core.exe' : 'creditia-core');
        const child = spawn(binary, ['desktop-api', command, '--json'], { cwd: ROOT, stdio: ['pipe','pipe','pipe'], env: process.env });
        let output = '', error = ''; child.stdout.on('data', chunk => output += chunk); child.stderr.on('data', chunk => error += chunk);
        child.on('error', e => { res.statusCode = 500; res.end(JSON.stringify({ error: e.message })); });
        child.on('close', code => { if (res.writableEnded) return; res.setHeader('content-type','application/json'); res.statusCode = code === 0 || code === 2 ? 200 : 500; res.end(output || JSON.stringify({ error: error || '本地核心没有返回结果' })); });
        child.stdin.end(body);
      });
    },
  };
}
export default defineConfig({
  publicDir: path.join(ROOT, 'packages/render/assets'), clearScreen: false,
  server: { host: '127.0.0.1', port: 5183, strictPort: true, fs: { allow: [ROOT] } },
  build: { target: 'safari15' }, plugins: [localCore()],
});

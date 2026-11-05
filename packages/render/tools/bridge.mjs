#!/usr/bin/env node
// One JSON request/response per line. Backpressure keeps only one active render
// and one bounded input line in memory; restart the child to cancel active work.
import { once } from 'node:events';
import { pathToFileURL } from 'node:url';
import { createRenderSession, errorResponse, MAX_REQUEST_BYTES } from '../src/protocol.ts';

export async function serve(input, output, backend) {
  const session = createRenderSession(backend);
  let pending = Buffer.alloc(0), discarding = false;
  async function send(response) {
    if (!output.write(JSON.stringify({ ...response, runtime: { rssBytes: process.memoryUsage.rss() } }) + '\n')) await once(output, 'drain');
  }
  async function line(bytes) {
    if (!bytes.length || !bytes.toString('utf8').trim()) return;
    let request;
    try { request = JSON.parse(bytes.toString('utf8')); }
    catch { await send(errorResponse(null, 'invalid_json', 'Request is not valid JSON')); return; }
    await send(await session.dispatch(request));
  }
  for await (const value of input) {
    const chunk = Buffer.isBuffer(value) ? value : Buffer.from(value);
    let start = 0;
    while (start < chunk.length) {
      const newline = chunk.indexOf(10, start);
      const end = newline < 0 ? chunk.length : newline;
      const part = chunk.subarray(start, end);
      if (!discarding) {
        if (pending.length + part.length > MAX_REQUEST_BYTES) {
          pending = Buffer.alloc(0); discarding = true;
          await send(errorResponse(null, 'request_too_large', 'Request exceeds 8 MiB'));
        } else pending = pending.length ? Buffer.concat([pending, part]) : Buffer.from(part);
      }
      if (newline >= 0) {
        if (!discarding) await line(pending);
        pending = Buffer.alloc(0); discarding = false;
      }
      start = end + 1;
    }
  }
  if (!discarding && pending.length) await line(pending);
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  const { nativeBackend } = await import('../src/native-protocol.ts');
  try { await serve(process.stdin, process.stdout, nativeBackend); }
  catch (error) {
    console.error(error instanceof Error ? error.message : String(error));
    process.exitCode = 1;
  }
}

#!/usr/bin/env node
// creditia 命令入口：发布的包用打包好的 dist；仓库里开发时直接跑源码（Node >= 22.18 能直接运行 .ts）
import { existsSync } from 'node:fs';
const dist = new URL('../dist/cli.mjs', import.meta.url);
await import(existsSync(dist) ? dist.href : new URL('../src/cli.ts', import.meta.url).href);

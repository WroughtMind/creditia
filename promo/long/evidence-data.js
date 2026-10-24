// pick.ts 开头 16 行（同短片）：谁写的，凭什么
//   verified 日志里有同样的一行（工具的会话记录）；declared 提交里写明（Creditia-* trailer）；
//   inferred 按改动推断；unattributed 未确认
import { FILE } from '../short/data.js';

const KIND = ['verified', 'verified', 'verified', 'declared', 'declared', 'inferred', 'verified', 'verified',
  'inferred', 'verified', 'none', 'declared', 'declared', 'declared', 'unattributed', 'unattributed'];
export const HERO = FILE.map(([text, cls], i) => ({ text, cls, kind: KIND[i] }));

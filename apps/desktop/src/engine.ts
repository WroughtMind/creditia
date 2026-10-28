// 每次导出独占一个 WASM 运行环境，结束后释放；一次底层异常不污染后续导出。
import type { Art } from '../../../packages/render/src/export.ts';
import type { Style } from '../../../packages/render/src/style.ts';

export type Format = 'png' | 'svg' | 'pdf' | 'gif' | 'html';
export type Exported = { bytes: Uint8Array; mime: string; ext: string };
export type ExportJob = { fmt: Format; fontStyle: Pick<Style, 'f'> | null; art: Art; size: string; frames?: Art[] };
/** 按格式出文件。frames 只在 GIF 时用 */
export async function exportArt(fmt: Format, st: Style | null, a: Art, size: string, frames?: () => Art[] | Promise<Art[]>): Promise<Exported> {
  const job: ExportJob = { fmt, fontStyle: st ? { f: st.f } : null, art: a, size };
  if (fmt === 'gif' && frames) job.frames = await frames();
  const worker = new Worker(new URL('./export-worker.ts', import.meta.url), { type: 'module' });
  return new Promise((resolve, reject) => {
    const fail = (error: unknown) => { worker.terminate(); reject(error); };
    worker.onmessage = ({ data }) => {
      worker.terminate();
      if (data.error) {
        const error = new Error(data.error.message);
        if (data.error.stack) error.stack = data.error.stack;
        reject(error);
      } else resolve(data.result);
    };
    worker.onerror = event => fail(new Error(event.message || 'Export renderer failed'));
    worker.onmessageerror = () => fail(new Error('Unable to read export result'));
    try { worker.postMessage(job); } catch (error) { fail(error); }
  });
}

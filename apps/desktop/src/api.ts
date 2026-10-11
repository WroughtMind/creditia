const native = (window as any).__TAURI__?.core?.invoke as ((command: string, args?: unknown) => Promise<any>) | undefined;
export async function launchContext(): Promise<{ repo?: string; theme?: string; form?: string }> {
  if (native) return native('launch_context');
  return Object.fromEntries(new URLSearchParams(location.search));
}
if (native && /Mac/.test(navigator.userAgent)) document.documentElement.classList.add('native-mac');
export interface Project { id: string; name: string; path: string; lastAnalyzed?: number; hasCredits?: boolean; exists?: boolean; paths?: string[] }
export function base64(bytes: Uint8Array) { let text = ''; for (let i = 0; i < bytes.length; i += 0x8000) text += String.fromCharCode(...bytes.subarray(i, i + 0x8000)); return btoa(text); }
export async function api(command: string, args: any = {}): Promise<any> {
  if (native) return native(command, args);
  const response = await fetch(`/__dev/${command}`, { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify(args) });
  const result = await response.json(); if (!response.ok) throw new Error(result.error ?? '本地请求失败'); return result;
}
export async function saveExport(name: string, bytes: Uint8Array, mime: string): Promise<string> {
  if (native) return native('save_file', { name, bytes: base64(bytes), mime });
  const url = URL.createObjectURL(new Blob([bytes as BlobPart], { type: mime }));
  const anchor = document.createElement('a'); anchor.href = url; anchor.download = name; anchor.click();
  setTimeout(() => URL.revokeObjectURL(url), 1000); return `已交给浏览器下载：${name}`;
}

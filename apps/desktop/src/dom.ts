export const $ = <T extends HTMLElement = HTMLElement>(selector: string) => document.querySelector(selector) as T;
export const esc = (text: unknown) => String(text ?? '').replace(/[&<>"']/g, c => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' })[c]!);
export const icon = (name: string, size = 18) => {
  const paths: Record<string, string> = {
    chevron: '<path d="m7 10 5 5 5-5"/>', plus: '<path d="M12 5v14M5 12h14"/>', close: '<path d="m6 6 12 12M6 18 18 6"/>',
    export: '<path d="M12 3v12m-4-4 4 4 4-4M5 16v4h14v-4"/>', adjust: '<path d="M4 7h8m4 0h4M4 17h2m4 0h10"/><circle cx="14" cy="7" r="2"/><circle cx="8" cy="17" r="2"/>',
    info: '<circle cx="12" cy="12" r="9"/><path d="M12 11v6m0-10v1"/>', refresh: '<path d="M20 7v5h-5M4 17v-5h5M5 8a8 8 0 0 1 13-3l2 3M4 16l2 3a8 8 0 0 0 13-3"/>',
    people: '<circle cx="9" cy="8" r="3"/><path d="M3 20v-2a6 6 0 0 1 12 0v2m1-15a3 3 0 0 1 0 6m3 9v-2a6 6 0 0 0-3-5"/>',
    copy: '<rect x="8" y="8" width="12" height="12" rx="2"/><path d="M16 8V4H4v12h4"/>', expand: '<path d="M4 9V4h5m6 0h5v5M4 15v5h5m6 0h5v-5"/>',
  }; return `<svg width="${size}" height="${size}" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">${paths[name] ?? paths.info}</svg>`;
};
let timer: ReturnType<typeof setTimeout>;
export function toast(message: string, persistent = false) { const el = $('#toast'); el.textContent = message; el.hidden = false; clearTimeout(timer); if (!persistent) timer = setTimeout(() => el.hidden = true, 4200); }
export async function copy(text: string) { await navigator.clipboard.writeText(text); toast('已复制'); }

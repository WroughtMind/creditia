// Narrow, reviewable compatibility rules. Never normalize arbitrary text,
// geometry, colors or filters. Callers must still prove decoded pixels match.
const badgeStacks = new Set([
  "Georgia, 'Times New Roman', serif", 'Menlo, Consolas, monospace',
  '-apple-system,Segoe UI,Helvetica,Arial,sans-serif', 'Georgia, serif',
  'Helvetica, Arial, sans-serif',
]);
export function portableBadgeReference(svg, row) {
  if (row.renderer !== 'classic' || row.piece !== 'badge') return { svg, changes: 0 };
  let changes = 0;
  const result = svg.replace(/<text\b([^>]*)>([\s\S]*?)<\/text>/g, (element, attributes, content) => {
    const stack = /font-family="([^"]+)"/.exec(attributes)?.[1];
    if (!badgeStacks.has(stack)) return element;
    const family = stack === 'Menlo, Consolas, monospace' && content.includes('❯') ? 'JetBrains Mono' : 'Noto Sans SC';
    changes++;
    return `<text${attributes.replace(/font-family="[^"]*"/, `font-family="${family}"`)}>${content}</text>`;
  });
  return { svg: result, changes };
}

// resvg 2.6.2 aborts the process for some nonintersecting text bounding boxes.
// Original terminal rain draws beyond the document. Drop only those glyphs
// whose 16px text lies more than 20px below the canvas; visible rain is untouched.
// The manifest explicitly records this accommodated reference, not a successful
// rasterization of the original baseline. Baseline source bytes stay immutable.
export function terminalRasterReference(svg, row, height) {
  if (row.renderer !== 'svg' || row.style !== 'terminal') return { svg, changes: 0 };
  let changes = 0;
  const result = svg.replace(/<text\b([^>]*)>[\s\S]*?<\/text>/g, (element, attributes) => {
    const attr = name => new RegExp(`\\b${name}="([^"]+)"`).exec(attributes)?.[1];
    if (attr('font-family') === 'DotGothic16, JetBrains Mono' && attr('font-size') === '16' &&
        attr('fill') === '#7CFF9E' && Number(attr('opacity')) <= .2 && Number(attr('y')) > height + 20) {
      changes++; return '';
    }
    return element;
  });
  return { svg: result, changes };
}

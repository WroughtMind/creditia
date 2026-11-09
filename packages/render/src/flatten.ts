/** resvg 2.6 serializes object-bounding-box gradients on outlined text with an
 * empty resource id. Resolve each affected text's own paint in isolation before
 * flattening the whole scene; never guess which anonymous gradient belongs to
 * which word. This only performs layout, with no RGBA rasterization. */
export function flattenSvg(svg: string, flatten: (svg: string) => string): string {
  const first = flatten(svg);
  if (!first.includes('url(#)')) return first;
  const defs = [...svg.matchAll(/<defs\b[^>]*>([\s\S]*?)<\/defs>/g)].map(m => m[1]).join('');
  const gradients = new Map([...defs.matchAll(/<(linearGradient|radialGradient)\b[^>]*\bid="([^"]+)"[^>]*>[\s\S]*?<\/\1>/g)].map(m => [m[2], m[0]]));
  let serial = 0;
  const additions: string[] = [];
  const updated = svg.replace(/<text\b[^>]*>[\s\S]*?<\/text>/g, (text, offset: number) => {
    let result = text;
    for (const match of [...text.matchAll(/\b(fill|stroke)="url\(#([^)]*)\)"/g)]) {
      const [attribute, paint, id] = match, original = gradients.get(id);
      if (!original || /gradientUnits="userSpaceOnUse"/.test(original)) continue;
      // Only this paint is present, so the resolved anonymous resource has one
      // unambiguous owner even when the scene has many gradient-filled titles.
      const isolatedText = text.replace(/\b(fill|stroke)="[^"]*"/g, (a, key) => key === paint ? attribute : `${key}="none"`);
      // Keep the real viewport and ancestor styles/transforms: inherited fonts
      // and percentage positions also determine the text's gradient coordinates.
      const isolated = flatten(svg.replace(/<text\b[^>]*>[\s\S]*?<\/text>/g,
        (_other, otherOffset: number) => otherOffset === offset ? isolatedText : ''));
      const resolved = [...isolated.matchAll(/<(linearGradient|radialGradient)\b[^>]*\bid=""[^>]*>[\s\S]*?<\/\1>/g)];
      if (resolved.length !== 1) continue;
      let name: string;
      do { name = `outlined-text-paint-${serial++}`; } while (svg.includes(`id="${name}"`));
      additions.push(resolved[0][0].replace('id=""', `id="${name}"`));
      result = result.replace(attribute, `${paint}="url(#${name})"`);
    }
    return result;
  });
  return additions.length ? flatten(updated.replace(/(<svg\b[^>]*>)/, `$1<defs>${additions.join('')}</defs>`)) : first;
}

/**
 * Whole ink plates used by the scroll atlas. All distances are in the original
 * artwork coordinates. The compositor moves the complete captured text group;
 * texture only changes existing ink, never clips a letter or a measured extent.
 *
 * These two functions are deliberately self-contained: the offline player
 * serializes them together with the single-raster compositor.
 */
export function scrollPaperPose(style: string, role: string, progress: number): {
  dx: number; dy: number; opacity: number;
  bleed?: number; bleedOpacity?: number; grain?: number; glint?: number;
} {
  const p = Math.max(0, Math.min(1, progress));
  if (p >= 1) return { dx: 0, dy: 0, opacity: 1 };
  const rest = 1 - p;
  const settle = rest * rest * (1 + 2 * p);
  const contact = Math.min(1, p / .08);

  if (style === 'ink') {
    // The unchanged ink face remains present while its wet perimeter contracts.
    // Bleed is a low-alpha copy outside SourceAlpha, not a blurred replacement.
    return { dx: 0, dy: 2 * settle, opacity: contact, bleed: 3.2 * rest * rest, bleedOpacity: .24, grain: 1 };
  }
  if (style === 'ukiyoe') {
    // The actual role and name plates approach their shared register from
    // opposite sides. No made-up strips, letters or colour ownership groups.
    const rolePlate = role === 'role' || role === 'label' || role === 'title';
    return { dx: (rolePlate ? -8 : 8) * settle, dy: -1.5 * settle, opacity: contact, grain: 1 };
  }
  if (style === 'woodblock') {
    // A complete printed line presses onto the page. Fine broken knife marks
    // fill into its continuous base ink; they never become barcode gaps.
    return { dx: 0, dy: -7 * settle, opacity: contact, bleed: 1.1 * rest * rest, bleedOpacity: .42, grain: 1 };
  }
  if (style === 'baroque') {
    // The real raised letter face settles; its glint is confined to existing
    // ink by the compositor and travels across the face, not across the paper.
    return { dx: 0, dy: -6 * settle, opacity: contact, glint: .9 * Math.sin(Math.PI * p) };
  }
  if (style === 'grimoire') {
    // Engraved lines interweave inside a complete dark base. No outline tracing
    // or floating decorative cross: the marks belong to the original glyphs.
    return { dx: 0, dy: 0, opacity: contact, grain: 1 };
  }
  if (style === 'manuscript') {
    // One complete written line meets its baseline with a little pen pressure;
    // stable short pencil strokes fill in, without handwriting individual letters.
    return { dx: 3.2 * settle, dy: -4.5 * settle, opacity: contact, bleed: 1.2 * rest, bleedOpacity: .35, grain: 1 };
  }
  return { dx: 0, dy: 0, opacity: 1 };
}

/** Stable pigment multiplier in original design coordinates; always >= .58. */
export function scrollPaperTexture(style: string, progress: number, x: number, y: number): number {
  const p = Math.max(0, Math.min(1, progress));
  if (p >= 1) return 1;
  const rest = 1 - p;

  if (style === 'ink') {
    // Soft paper fibres pool the wet ink. Positions stay fixed while the wet
    // edge contracts; there is no frame-random shimmer or cellular cutout.
    const fibre = .5 + .5 * Math.sin(x * .63 + Math.sin(y * .31) * 1.4);
    return 1 - .15 * rest * rest * fibre;
  }
  if (style === 'ukiyoe') {
    // Broad, low-contrast rubbing pressure on the whole registered plate.
    const pressure = .5 + .5 * Math.sin(y * .19 + Math.sin(x * .037) * .6);
    return 1 - .13 * rest * pressure;
  }
  if (style === 'woodblock') {
    // Thin, irregular, broken knife tracks; the full ink face is never removed.
    const a = ((x + .5 * Math.sin(y * .28)) % 17 + 17) % 17;
    const b = ((y + Math.floor(x / 17) * 7) % 31 + 31) % 31;
    const cut = (a < 1.1 && b < 17) || (a > 7.1 && a < 8.15 && b > 8) || (a > 12.7 && a < 13.5 && (b < 10 || b > 20));
    return cut ? 1 - .36 * rest : 1;
  }
  if (style === 'grimoire') {
    // Two fine engraved directions. Between them the continuous base is still
    // readable, including small model names and numbers.
    const a = ((x + y) % 13 + 13) % 13;
    const b = ((x - y) % 13 + 13) % 13;
    const hatch = a < 1.4 ? 1 : b < 1.2 ? Math.max(0, Math.min(1, (p - .1) / .4)) : 0;
    return 1 - .35 * rest * (1 - hatch);
  }
  if (style === 'manuscript') {
    // Short diagonal graphite strokes, with varying endpoints so they do not
    // resemble a regular screen over the writing.
    const cell = Math.floor(y / 9);
    const a = ((x + y * 1.7 + cell * 5) % 23 + 23) % 23;
    const b = ((y + Math.floor(x / 23) * 3) % 9 + 9) % 9;
    const pencil = a < 1.5 && b > 1 && b < 6;
    return pencil ? 1 : 1 - .36 * rest;
  }
  return 1;
}

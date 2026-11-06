import { stable, sha256, pixelDiff } from './common.mjs';
import { decodePng, decodePdf, decodeGif, firstSvg, quantizedDiff } from './export-validation.mjs';
function unwrap(response) { if (!response.ok) throw Error(`${response.error.code}: ${response.error.message}`); return response.result; }
export async function exportChecks(candidate, request, row, rowIndex, captureFailure, onStage) {
  // Every row exercises all encoders once, with preferences/transparency rotated
  // across the matrix. The four preference variants are independently compared
  // above; repeating these same encoder paths for each adds no format coverage.
  const selected = { ...request, transparent: rowIndex % 2 === 1 };
  onStage?.('reference-art');
  const original = unwrap(await candidate.native.renderRequest(selected)).art;
  if (!original) throw Error('Export fixture does not produce an artwork');
  const size = '320x240', canvas = { w: 320, h: 240 }, frameCount = 4;
  const fit = art => candidate.exports.sized(art, canvas.w, canvas.h);
  const outlined = art => art.svg.includes('<text') ? candidate.raster(art.svg).flat(art.svg) : art.svg;
  const fitted = fit(original), expected = candidate.raster(fitted.svg).rgba(fitted.svg, canvas.w);
  const animation = row.animated === 'true' ? unwrap(await candidate.native.renderRequest({ ...selected, op: 'frames', frameCount })) : null;
  const formats = ['png', 'svg', 'pdf', 'html', ...(animation ? ['gif'] : [])], results = [];
  for (const format of formats) {
    onStage?.(`encode-${format}`);
    const exported = unwrap(await candidate.native.renderRequest({ ...selected, op: 'export', format, size, frameCount, width: 160 }));
    onStage?.(`verify-${format}`);
    const bytes = Buffer.from(exported.bytes, 'base64'), checks = {};
    if (format === 'png') {
      const decoded = decodePng(bytes);
      checks.dimensions = decoded.w === canvas.w && decoded.h === canvas.h;
      checks.pixels = pixelDiff(expected, decoded);
    } else if (format === 'svg') {
      const svg = bytes.toString();
      const decoded = candidate.raster(svg).rgba(svg, canvas.w);
      checks.dimensions = decoded.w === canvas.w && decoded.h === canvas.h;
      checks.outlined = !svg.includes('<text');
      checks.accessible_title = svg.includes('role="img"') && svg.includes('aria-label="');
      checks.pixels = pixelDiff(candidate.raster(outlined(fitted)).rgba(outlined(fitted), canvas.w), decoded);
    } else if (format === 'pdf') {
      const decoded = decodePdf(bytes), scale = Math.min(3, 4000 / Math.max(canvas.w, canvas.h));
      checks.page_dimensions = decoded.page_width_points === canvas.w * .75 && decoded.page_height_points === canvas.h * .75;
      checks.linked_single_page = decoded.objects >= 6;
      checks.pixels = pixelDiff(candidate.raster(fitted.svg).rgba(fitted.svg, canvas.w * scale), decoded);
    } else if (format === 'html') {
      const html = bytes.toString(), still = fit(animation?.frames.at(-1) ?? original), embedded = firstSvg(html);
      checks.selected_language = html.includes(`<html lang="${request.lang}">`);
      checks.single_file = !/(?:src|href)=["']https?:\/\//i.test(html);
      checks.pixels = pixelDiff(candidate.raster(outlined(still)).rgba(outlined(still), canvas.w), candidate.raster(embedded).rgba(embedded, canvas.w));
      const data = /const DATA=(.*);\r?\n/.exec(html);
      checks.hover_geometry = !!data && stable(JSON.parse(data[1])) === stable([{ w: still.w, h: still.h, hits: still.hits }]);
      checks.reduced_motion_handler = html.includes('prefers-reduced-motion');
      if (animation) {
        const player = /\nhtmlPlayer\((.*)\);\r?\n/.exec(html), payload = player ? JSON.parse(player[1]) : null;
        checks.shared_timeline = !!payload && stable(payload.times) === stable(animation.times) && stable(payload.timeline) === stable(animation.timeline);
        checks.sample_count = payload?.svgs?.length === animation.frames.length;
        checks.sampled_pixels = checks.sample_count && payload.svgs.every((svg, i) => {
          const frame = fit(animation.frames[i]);
          return pixelDiff(candidate.raster(outlined(frame)).rgba(outlined(frame), 160), candidate.raster(svg).rgba(svg, 160)).equal;
        });
      }
    } else if (format === 'gif') {
      const decoded = decodeGif(bytes), timeline = animation.timeline;
      checks.dimensions = decoded.w === 160 && decoded.h === 120;
      checks.sample_count = decoded.frames.length === animation.frames.length;
      checks.loop = decoded.loop === (timeline.loop ? 0 : null);
      checks.shared_delays = checks.sample_count && decoded.frames.every((frame, i) => {
        const endpoint = time => Math.round(time * timeline.durationMs / 10);
        const ticks = i + 1 < animation.times.length ? endpoint(animation.times[i + 1]) - endpoint(animation.times[i]) : Math.round(timeline.holdMs / 10);
        return frame.delay_ms === Math.max(1, ticks) * 10;
      });
      checks.frame_pixels = checks.sample_count ? decoded.frames.map((frame, i) => {
        const art = fit(animation.frames[i]); return quantizedDiff(candidate.raster(art.svg).rgba(art.svg, 160), frame);
      }) : [];
    }
    const passed = value => typeof value === 'boolean' ? value : Array.isArray(value) ? value.length > 0 && value.every(passed) : value?.equal === true;
    const result = { format, size, reference_transform: ['svg', 'html'].includes(format) ? 'Explicit text-to-path outline; already outlined paths preserved without reserialization' : 'Fitted source artwork', checks, passed: Object.values(checks).every(passed), bytes: bytes.length, sha256: sha256(bytes) };
    if (!result.passed && captureFailure) {
      result.artifact = captureFailure(`actual.${format}`, bytes);
      result.reference = captureFailure(`${format}-reference.svg`, ['svg', 'html'].includes(format) ? outlined(format === 'html' && animation ? fit(animation.frames.at(-1)) : fitted) : fitted.svg);
    }
    results.push(result);
  }
  return { status: results.every(r => r.passed) ? 'matched' : 'different', preferences: { lang: request.lang, group: request.group, named: request.named }, transparent: selected.transparent,
    canvas, frame_count_parameter: frameCount, formats: results,
    limitation: 'Independent PNG/GIF decoding, PDF structure and image streams, SVG/HTML raster and hover/timeline invariants; CLI/browser invocation and native file dialogs need separate acceptance. GIF palette error budget is explicit per frame.' };
}

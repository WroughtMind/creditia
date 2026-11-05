# Native renderer protocol v1

The GPUI desktop can call the existing Creditia artwork renderer without a
WebView. `src/protocol.ts` contains the environment-independent dispatcher;
`src/native-protocol.ts` supplies Node/Satori/resvg and compression. Browser
workers can inject their own `RenderBackend` into the same dispatcher.

Install the existing package dependencies with `npm ci`, then run:

```sh
node packages/render/tools/bridge.mjs
```

Run that command from the repository root; from this package use `npm run bridge`.
It requires Node 22.18 or later. Each input line is one UTF-8 JSON request. Each
output line is one response with the same `id` and `version`. The process exits
after stdin EOF. It handles requests sequentially, including stdout backpressure;
the host may start one child for each request or keep a child alive.

The native host must execute Node directly with an argument array, supervise the
child, and terminate/reap it on cancellation, timeout or window close. This bridge
does not cancel a synchronous Satori/resvg call internally. Request IDs allow the
host to discard stale results after project, style or selection changes.

## Requests

```json
{"version":1,"id":"catalog","op":"catalog"}
```

The catalog contains 26 visible desktop styles: 15 classic themes and 11 SVG
styles. Each entry has `id`, Chinese `name`, English `en`, `renderer`, and `pieces`.
Pieces contain `variants` (objects with `id`, `name`, `en`), `defaultVariant`,
`animated`, and `requires`. Classic `profile` needs `projects`; `blame` needs a
`blame` result. These two contextual pieces are included in the shared registry
even though they are not on the current desktop wall. The catalog also reports
supported graphic `formats`, canvas `sizes`, and protocol limits.

```json
{
  "version": 1,
  "id": 2,
  "op": "render",
  "style": "minimal-light",
  "piece": "tier",
  "variant": "original",
  "raw": {"schemaVersion": 0, "project": {"name": "Example"}},
  "lang": "zh",
  "group": "auto",
  "named": true,
  "flatten": true
}
```

`raw` is the actual credits archive, not a renderer-specific model. `lang`
defaults to `zh`; `group` defaults to `auto` (`tool`/`model` are also accepted);
`named` defaults to true. Omit `variant` to preserve the selected style's default.
`flatten: true` converts text to paths with the existing bundled fonts, which is
necessary for consistent native SVG previews. It does not rasterize the artwork.
The result is `{ "art": Art | null }`; `null` means the archive lacks data for
that artwork. `Art` keeps dimensions, the full-color SVG, hit regions, background,
title, and any motion/crop metadata.

An optional `t` in `[0,1]` samples the same motion used for GIF frames. Classic
layouts use the existing reveal/rise implementation. Bridged and SVG pieces use
`registry.frame`, including the scrolling credits viewport and crop definition.
Omitting `t` produces the complete static artwork. This exposes the mainline
motion; it does not claim the planned new style-specific animations are complete.

`projects` accepts up to 50 archive objects for classic profile artwork; `person`
is an optional label and `blame` is the existing blame result object.

For an export, use the same artwork selection with `op: "export"` and:

```json
{"format":"png","size":"native","frameCount":24}
```

Formats are `png`, `svg`, `pdf`, `gif`, and `html`. Sizes are `native`, `og`,
`square`, `portrait`, `story`, and `a4`. `frameCount` is a count of intervals
between animation samples (1–120, default 24); scrolling credits retain their
existing minimum of 60 intervals. `t` and `flatten` are render-only options.
GIF is rejected for a nonanimated piece. The result is:

```json
{"bytes":"BASE64_BYTES","encoding":"base64","mime":"image/png","ext":"png"}
```

The host owns the file dialog and file writing. The renderer accepts no input
SVG, paths, shell commands or output filenames. PNG/SVG/PDF/HTML/GIF use the same
`makeExporter` as CLI and desktop. Existing HTML behavior is preserved: a
scrolling credits artwork retains its CSS motion; other pieces use the existing
HTML entrance effect. HTML is not yet an arbitrary sampled timeline container.
Markdown, text and archive export remain in the existing text/CLI/schema modules.

## Limits and failures

Requests are limited to 8 MiB per line and responses to 64 MiB. Bitmap allocation
is checked before resvg: at most 32 million pixels and 32,768 pixels per edge.
Very tall artwork can still be exported as SVG/HTML. Unknown fields and invalid
style/piece/layout IDs are errors rather than silent fallbacks. All responses use:

```json
{"version":1,"id":2,"ok":false,"error":{"code":"unknown_style","message":"Unknown style: example"}}
```

Malformed or oversized input has `id: null`. An oversized line is discarded up
to its newline, so the next request can succeed. A renderer exception returns
`render_failed`; transport startup failures (including missing Node dependencies)
exit nonzero and must be presented by the native host. There is no automatic
fallback to system fonts or a new artwork design.

This is the shared rendering boundary for migration. Packaging Node, fonts and
the native resvg dependency into platform installers, GPUI texture caching,
native hit testing, real-time animation scheduling, reduced-motion behavior,
transparent-background overrides and visual/performance acceptance remain
separate work. Passing the protocol tests is not full desktop migration approval.

## Verification

```sh
npm ci
node --test test/protocol.test.ts
npm test
```

The focused test uses the committed Catnap archive, checks all 26 style IDs,
compares shared output and hit regions, compares exact sampled motion, checks
the five encoded formats, and exercises JSONL failure recovery and EOF.

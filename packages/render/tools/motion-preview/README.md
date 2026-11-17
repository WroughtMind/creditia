# Motion review tools

Install renderer and desktop dependencies using their package locks. Use Node 22.18+ and install FontTools so `pyftsubset` is on PATH (or set `PYFTSUBSET`). Pillow is used to inspect the actual scroll GIF frames.

```sh
npm --prefix packages/render ci
npm --prefix apps/desktop ci
npm --prefix apps/desktop run fonts
node packages/render/tools/motion-preview/build.mjs
node packages/render/tools/motion-preview/verify-matrix.mjs
```

The builder writes a self-contained HTML preview to `output/playwright/issue116-motion-preview.html`.

The matrix covers all 1523 product combinations: 473 modern, 435 bridged, and 615 native Satori layouts. It records four real raster samples per combination, validates finite geometry and hits, requires visible pixel changes, and compares the final frame to the static artwork. Scroll samples exercise the public GIF exporter and decode its cropped frames, keeping the single-rasterization path; they are not re-rendered offscreen SVGs.

Results, PNGs and incremental checkpoints are saved in `output/playwright/material-matrix/`. Each result records the source commit and diff hash, fixture hash, raster width, sample times, and image/SVG hashes. This is a correctness check; it does not replace viewing motion or native Mac acceptance.

Optional flags:

- `--baseline /path/to/checkout`: compare every final static artwork against an immutable older checkout. Identical normalized SVGs establish equality directly; differing SVGs must produce identical pixels. The baseline commit is recorded.
- `--font-directory output/playwright/.motion-preview-cache/fonts`: use the preview font subsets. A strict pixel comparison against the original exporter runs for all 26 styles before any cases; a mismatch stops the check.
- `--shard 1/3`: run one interleaved partition, writing distinct result/checkpoint files. Run all partitions and check their total is 1523.
- `--filter style/piece`: select cases for a targeted rerun. The result explicitly records the smaller selected count.
- `--output /path`: place a rerun's results in a separate directory.

See `HANDOFF.md` for the original handoff and its historical acceptance boundary.

# Motion review tools

Install renderer and desktop dependencies using their package locks. Use Node 22.18+ and install FontTools so `pyftsubset` is on PATH (or set `PYFTSUBSET`).

```sh
node packages/render/tools/motion-preview/build.mjs
node packages/render/tools/motion-preview/verify-matrix.mjs
```

The builder writes a self-contained HTML preview to `output/playwright/issue116-motion-preview.html`. The matrix checks the 473 modern plus 435 bridged combinations and writes incremental results to `output/playwright/material-matrix/`. It does not cover the 615 original Satori layouts or replace visual inspection.

See `packages/render/tools/motion-preview/HANDOFF.md` for the current acceptance boundary and remaining work.

# Creditia's resvg patch

Upstream: resvg 0.48.1, https://github.com/linebender/resvg, under MIT or Apache-2.0.
Copied from the crate pinned by the GPUI application's lockfile. The library source
and upstream license files are retained; the CLI/examples are not packaged here.

The local patch adds an opt-in, bounded cache for non-stitching `feTurbulence`
noise. Noise depends on its seed, frequencies, octaves, scale and coordinates;
it does not depend on the animated source image or displacement amount. Cache
blocks use exactly the upstream sampling algorithm. Stitching noise continues
through the original algorithm because its result depends on the filter bounds.

It also corrects the displacement caller's scale: the pixel primitive already
multiplies by the SVG `scale` value, so its caller must pass only the canvas-axis
scale. Passing the authored value twice squares the amount and loses the sign;
parchment edges then turn into large holes through the artwork. The regression
test checks positive, zero and negative displacement at uniform and nonuniform
display scales against the [SVG displacement equation](https://www.w3.org/TR/filter-effects-1/#feDisplacementMapElement).

Masks, blending, color conversion and SVG output otherwise remain owned by the
upstream implementation. No effect is removed or replaced.

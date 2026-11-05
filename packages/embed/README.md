# Creditia web artwork

Build with `npm run build` and serve the contents of `dist` together. The module
loads its bundled layout engine and pinned fonts relative to `creditia-card.js`;
it does not require a font CDN.

```html
<script type="module" src="/creditia/creditia-card.js"></script>
<creditia-card src="/credits.json" renderer="svg" theme="ink"
  piece="tier" variant="grouped" lang="zh" group="model"></creditia-card>
```

`renderer`, `theme`, `piece`, and `variant` use the shared artwork catalog.
`named`, `sign`, and `transparent` have the same meaning as desktop exports.
Use `animated` to play the shared motion definition; the element exposes
`play()`, `pause()`, and `replay()`, pauses while hidden, and honors the system's
reduced-motion preference.

An application may assign an immutable copy of the complete artwork source
instead of fetching `src`. This also supplies cross-project and evidence context:

```js
const card = document.createElement('creditia-card');
card.setAttribute('renderer', 'classic');
card.setAttribute('theme', 'minimal-light');
card.setAttribute('piece', 'profile');
card.data = { raw: archive, projects: [archive, anotherArchive], person: 'taek' };
document.body.appendChild(card);
// For an evidence artwork, use { raw: archive, blame: evidence }.
```

Assigning new data interrupts pending work. Setting `data = null` or changing
`src` resumes URL loading. The `creditia-ready` event contains `detail.art`
(or `null` for a valid archive without the required data); failures emit
`creditia-error`. Animation preparation emits `creditia-animation-ready` or
`creditia-animation-error`.

The browser acceptance tool renders every catalog selection through this actual
custom element and compares canonical SVG, dimensions, title, and hover regions
with the native shared renderer. It separately exercises playback and preference
changes, and retains representative browser screenshots.

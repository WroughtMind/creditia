"""Adapt the pinned official marks to the Hero's single foreground ink."""
import base64
import io
import json
from pathlib import Path
import sys
import xml.etree.ElementTree as ET

from PIL import Image

ET.register_namespace('', 'http://www.w3.org/2000/svg')
ASSETS = Path(__file__).parent / 'assets' / 'agents'


def icon(key, spec):
    file = ASSETS / spec['file']
    if file.suffix != '.svg':
        im = Image.open(file).convert('RGBA')
        if 'viewBox' in spec:
            x, y, w, h = map(int, spec['viewBox'].split())
            im = im.crop((x, y, x + w, y + h))
        mode = spec['mask']
        pixels = []
        for r, g, b, a in im.getdata():
            light = (0.2126 * r + 0.7152 * g + 0.0722 * b) / 255
            factor = {'alpha': 1, 'light': light, 'dark': 1 - light,
                      'white': max(0, min(r, g, b) - 220) / 35}[mode]
            pixels.append((255, 255, 255, round(a * factor)))
        im.putdata(pixels)
        im.thumbnail((96, 96), Image.Resampling.LANCZOS)
        buf = io.BytesIO()
        im.save(buf, 'PNG')
        data = base64.b64encode(buf.getvalue()).decode()
        # Alpha masks preserve the source silhouette; only currentColor is painted.
        return (f'<svg viewBox="0 0 {im.width} {im.height}" aria-hidden="true">'
                f'<defs><mask id="hero-{key}" mask-type="alpha">'
                f'<image width="{im.width}" height="{im.height}" href="data:image/png;base64,{data}"/>'
                f'</mask></defs><rect width="100%" height="100%" fill="currentColor" '
                f'mask="url(#hero-{key})"/></svg>')

    root = ET.fromstring(file.read_text())
    for parent in root.iter():
        for node in list(parent):
            tag = node.tag.split('}')[-1]
            if (tag in ('style', 'title', 'metadata')
                    or key in ('jazz', 'reasonix') and tag == 'rect'
                    or key == 'eve' and node.get('d') == 'M0 0h102v102H0z'
                    or key == 'mcpjam' and node.get('fill') == '#2D2D2D'
                    or key == 'inference-sh' and tag == 'linearGradient'
                    or key == 'tinycloud' and node.get('class', '').startswith('d1-')
                    or key == 'tabnine' and node.get('class') == 'letter'):
                parent.remove(node)
    ids = {node.get('id'): f'hero-{key}-{node.get("id")}'
           for node in root.iter() if node.get('id')}
    for node in root.iter():
        for attr, value in list(node.attrib.items()):
            if attr in ('fill', 'stroke') and value not in ('none', 'transparent'):
                node.set(attr, 'currentColor')
            elif attr == 'id':
                node.set(attr, ids[value])
            elif attr in ('style', 'class'):
                del node.attrib[attr]
            else:
                for old, new in ids.items():
                    value = value.replace(f'url(#{old})', f'url(#{new})')
                    if value == f'#{old}':
                        value = f'#{new}'
                node.set(attr, value)
        if key == 'augment' and node.tag.endswith('path'):
            node.set('fill', 'currentColor')
    for attr in ('width', 'height', 'id', 'version', 'role'):
        root.attrib.pop(attr, None)
    if spec.get('viewBox'):
        root.set('viewBox', spec['viewBox'])
    root.set('aria-hidden', 'true')
    # Explicit root fill is essential for outline-only Tabnine and Rovo Dev.
    if 'fill' not in root.attrib:
        root.set('fill', 'currentColor')
    return ET.tostring(root, encoding='unicode')


print(json.dumps({key: icon(key, spec) for key, spec in json.load(sys.stdin).items()}))

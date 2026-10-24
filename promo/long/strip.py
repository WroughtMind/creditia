# 片尾字幕：creditia 导出的三种 cinema 片尾版式（出品、各工具各模型、THE END），裁掉四周的黑，竖着接成一条长字幕
# THE END 下面那五颗金色菱形从图里抠掉，记下位置：片子里它们是单独的网格，最后变成星号的五瓣
#   python3 promo/long/strip.py promo/.build/long/roll promo/.build/long/tex
import json, os, sys
import numpy as np
from PIL import Image
from scipy import ndimage

src, dst = sys.argv[1], sys.argv[2]
os.makedirs(dst, exist_ok=True)
ORDER = ['production', 'crew', 'end']
S = 0.72          # 存下来的比例：1080p 下字差不多是一比一
GAP = 420         # 段与段之间的黑（原图像素）

parts = []
for v in ORDER:
    a = np.asarray(Image.open(os.path.join(src, f'{v}.png')).convert('RGB')).astype(np.int16)
    # 版式自己的底色（近黑、略带蓝）压成纯黑，接起来看不出一块块的边
    bg = np.median(a[:40, :40].reshape(-1, 3), axis=0)
    a = np.clip((a - bg) * 255.0 / (255 - bg), 0, 255).astype(np.int16)
    lit = a.max(axis=2) > 28
    ys, xs = np.where(lit)
    y0, y1 = max(0, ys.min() - 40), min(a.shape[0], ys.max() + 40)
    parts.append((v, a[y0:y1], (xs.min(), xs.max())))

W = max(p[1].shape[1] for p in parts)
H = sum(p[1].shape[0] for p in parts) + GAP * (len(parts) - 1) + GAP * 2
strip = np.zeros((H, W, 3), np.int16)
y = GAP
meta = {'sections': {}}
for v, a, _ in parts:
    x = (W - a.shape[1]) // 2
    if v == 'end':
        r, g, b = a[..., 0], a[..., 1], a[..., 2]
        gold = (r > 120) & (g > 80) & (b < r - 50) & (r - g < 90)
        lab, n = ndimage.label(gold)
        objs = ndimage.find_objects(lab)
        blobs = []
        for i, sl in enumerate(objs):
            h, w = sl[0].stop - sl[0].start, sl[1].stop - sl[1].start
            area = (lab[sl] == i + 1).sum()
            # 菱形：宽高相近，填满外接框的一半左右
            if 6 < h < 80 and 0.8 < w / max(1, h) < 1.25 and 0.38 < area / (h * w) < 0.66:
                blobs.append(((sl[0].start + sl[0].stop) / 2, (sl[1].start + sl[1].stop) / 2, max(h, w), area, sl))
        # 同一行、差不多大、等距的五颗；有几行就取最上面那行（紧挨着 THE END）
        best = None
        for cy, cx, sz, ar, sl in blobs:
            row = sorted([bb for bb in blobs if abs(bb[0] - cy) < 4 and abs(bb[2] - sz) < 5], key=lambda bb: bb[1])
            if len(row) != 5:
                continue
            gaps = np.diff([bb[1] for bb in row])
            if gaps.max() - gaps.min() > 4:
                continue
            if best is None or cy < best[0][0]:
                best = row
        assert best, f'没找到五颗菱形：{len(blobs)} 个候选'
        dia = []
        for cy, cx, sz, ar, sl in best:
            m = ndimage.binary_dilation(lab[sl] > 0, iterations=3)
            ys_, xs_ = sl
            pad = 4
            a[ys_.start - pad:ys_.stop + pad, xs_.start - pad:xs_.stop + pad] = 0
            dia.append({'x': (x + cx) * S, 'y': (y + cy) * S, 'size': sz * S})
        meta['diamonds'] = dia
        col = a[best[0][4][0].start - 2:best[0][4][0].stop + 2, best[0][4][1].start - 2:best[0][4][1].stop + 2]
    strip[y:y + a.shape[0], x:x + a.shape[1]] = a
    meta['sections'][v] = {'y0': y * S, 'y1': (y + a.shape[0]) * S}
    y += a.shape[0] + GAP

img = Image.fromarray(np.clip(strip, 0, 255).astype(np.uint8))
img = img.resize((round(W * S), round(H * S)), Image.LANCZOS)
img.save(os.path.join(dst, 'roll.jpg'), quality=93)
meta.update({'w': img.width, 'h': img.height})
json.dump(meta, open(os.path.join(dst, 'roll.json'), 'w'), indent=1)
print('strip →', img.size, meta['diamonds'][0])

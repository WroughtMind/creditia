# 署名卡（minimal-dark 的 share）上六条份额条的确切位置：片子里颜色流进来的落点就是这几条
#   python3 promo/long/cardbars.py promo/.build/long/card.png promo/.build/long/tex/card.json
import json, sys
import numpy as np
from PIL import Image

a = np.asarray(Image.open(sys.argv[1]).convert('RGB')).astype(int)
H, W = a.shape[:2]
track = np.array([38, 38, 37])
x_probe = int(W * 0.8)
col = a[:, x_probe]
rows, run = [], []
for y in range(H):
    if np.abs(col[y] - track).sum() < 8:
        run.append(y)
    elif run:
        rows.append(run); run = []
bars = []
for r in rows:
    if not 6 <= len(r) <= 30:
        continue
    yc = (r[0] + r[-1]) / 2
    row = a[int(yc)]
    tr = [x for x in range(W // 2, W) if np.abs(row[x] - track).sum() < 8]
    x1 = max(tr)
    # 填充：从条的左端往右，和底色不同的那一段
    # 条的左端：卡片（右半边那块面板）里第一个不是面板底色的点
    card = a[int(yc) - 20, int(W * 0.53)]
    x0 = next(x for x in range(int(W * 0.53), x1) if np.abs(row[x] - card).sum() > 10)
    fill = [x for x in range(x0, x1) if np.abs(row[x] - track).sum() >= 25 and np.abs(row[x] - card).sum() >= 25]
    xf = max(fill) if fill else x0
    bars.append({'y': yc / H, 'h': len(r) / H, 'x0': x0 / W, 'x1': x1 / W, 'fill': xf / W, 'color': '#%02x%02x%02x' % tuple(row[x0 + 3])})
json.dump({'w': W, 'h': H, 'bars': bars}, open(sys.argv[2], 'w'), indent=1)
print(len(bars), 'bars', [round(b['fill'] - b['x0'], 3) for b in bars])

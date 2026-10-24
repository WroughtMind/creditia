# 扇面里的贴图：26 张作品图缩到长边 1400（1080p 下一张牌最多占三分之一屏宽，够清楚）
import glob, os, sys
from PIL import Image
src, dst = sys.argv[1], sys.argv[2]
os.makedirs(dst, exist_ok=True)
for f in sorted(glob.glob(os.path.join(src, '*.png'))):
    out = os.path.join(dst, os.path.basename(f)[:-4] + '.jpg')
    if os.path.exists(out) and os.path.getmtime(out) > os.path.getmtime(f):
        continue
    im = Image.open(f).convert('RGB'); w, h = im.size; s = min(1.0, 1400 / max(w, h))
    im.resize((round(w * s), round(h * s)), Image.LANCZOS).save(out, quality=92)

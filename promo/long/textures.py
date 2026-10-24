# 贴图：看样台上的 26 件缩到长边 1400；署名卡（近景要看清）长边 2048；片尾字幕 strip.py 自己出
# 另有一套 -lo（一半大小），低分辨率预览时用，省内存
import glob, os, sys
from PIL import Image
b = sys.argv[1] if len(sys.argv) > 1 else 'promo/.build/long'
os.makedirs(f'{b}/tex/prints', exist_ok=True)
os.makedirs(f'{b}/tex/prints-lo', exist_ok=True)
def shrink(f, out, edge):
    if os.path.exists(out) and os.path.getmtime(out) > os.path.getmtime(f):
        return
    im = Image.open(f).convert('RGB'); w, h = im.size; s = min(1.0, edge / max(w, h))
    im.resize((round(w * s), round(h * s)), Image.LANCZOS).save(out, quality=92)
for f in sorted(glob.glob(f'{b}/prints/*.png')):
    shrink(f, f'{b}/tex/prints/' + os.path.basename(f)[:-4] + '.jpg', 1400)
    shrink(f, f'{b}/tex/prints-lo/' + os.path.basename(f)[:-4] + '.jpg', 640)
shrink(f'{b}/card.png', f'{b}/tex/card.jpg', 2048)
shrink(f'{b}/card.png', f'{b}/tex/card-lo.jpg', 1024)

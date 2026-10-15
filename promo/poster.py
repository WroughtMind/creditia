"""README 封面：成片里的一帧，压暗一点，正中一个播放键。python3 promo/poster.py in.png out.jpg"""
import sys

from PIL import Image, ImageDraw, ImageFilter

im = Image.open(sys.argv[1]).convert("RGB").resize((1280, 720), Image.LANCZOS)
ov = Image.new("RGBA", im.size, (0, 0, 0, 0))
d = ImageDraw.Draw(ov)
cx, cy, r = 640, 360, 58
d.ellipse([cx - r, cy - r, cx + r, cy + r], fill=(7, 7, 8, 150), outline=(241, 234, 220, 210), width=3)
d.polygon([(cx - 18, cy - 28), (cx - 18, cy + 28), (cx + 30, cy)], fill=(241, 234, 220, 235))
shadow = ov.filter(ImageFilter.GaussianBlur(1))
im = Image.alpha_composite(im.convert("RGBA"), shadow).convert("RGB")
im.save(sys.argv[2], quality=88, optimize=True, progressive=True)
print("poster →", sys.argv[2])

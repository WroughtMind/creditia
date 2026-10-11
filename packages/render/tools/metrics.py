# 生成 src/metrics.json：每个字体的 ASCII 与常用符号的字宽（1/1000 em）、CJK 字宽、大写字高。
# 用法：python3 tools/metrics.py（读 node_modules 里的字体，清单来自 src/fonts.ts 的导出 JSON）
import json, os, sys, subprocess
from fontTools.ttLib import TTFont
HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(HERE)
faces = json.loads(subprocess.check_output(['node', '-e', "import('./src/fonts.ts').then(m=>console.log(JSON.stringify(m.FONTS)))"], cwd=ROOT))
CH = [chr(c) for c in range(32, 127)] + list('·—–…‘’“”•×→←↑↓■□◆◇★☆●○%«»§¶†‡°±÷≈≠≤≥№€£¥©®™‰√∞')
out = {}
for f in faces:
    d = os.path.join(ROOT, 'node_modules/@expo-google-fonts', f['pkg'], f['dir'])
    ttf = next(x for x in os.listdir(d) if x.endswith('.ttf'))
    t = TTFont(os.path.join(d, ttf), lazy=True)
    upm = t['head'].unitsPerEm
    cmap = t.getBestCmap(); hmtx = t['hmtx']
    def adv(ch):
        g = cmap.get(ord(ch))
        return round(hmtx[g][0] * 1000 / upm) if g else None
    w = {}
    for ch in CH:
        a = adv(ch)
        if a is not None: w[ch] = a
    cjk = adv('中') or adv('永')
    os2 = t['OS/2']
    cap = getattr(os2, 'sCapHeight', 0) or round(upm * 0.7)
    key = f"{f['family']}|{f['weight']}{'i' if f.get('italic') else ''}"
    out[key] = {'w': w, 'cjk': cjk, 'cap': round(cap * 1000 / upm), 'asc': round(t['hhea'].ascent * 1000 / upm), 'desc': round(-t['hhea'].descent * 1000 / upm)}
json.dump(out, open(os.path.join(ROOT, 'src/metrics.json'), 'w'), ensure_ascii=False, separators=(',', ':'))
print(len(out), 'faces', os.path.getsize(os.path.join(ROOT, 'src/metrics.json')), 'bytes')

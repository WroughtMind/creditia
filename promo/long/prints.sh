#!/usr/bin/env bash
# 看样台上的作品：同一份署名（样例项目 catnap），26 种风格各一件，件件不同
set -euo pipefail
cd "$(dirname "$0")/../.."
OUT=promo/.build/long/prints
mkdir -p $OUT
C=packages/render/fixtures/catnap.json
while read -r style piece variant; do
  [ -z "$style" ] && continue
  f=$OUT/$style.png
  [ -f $f ] && continue
  node packages/cli/bin/creditia.mjs export --credits $C --piece $piece --style $style ${variant:+--variant $variant} --lang en --scale 2 --out $f >/dev/null 2>&1
done <<'LIST'
minimal-light share
minimal-dark codemap
glass donut
film roll scroll
cinema tier
editorial versions
broadsheet areas
playbill receipt
gala waffle
mono calendar
poster share
terminal fetch
crt lanes
neon tier
arcade clock
ink share
ukiyoe share
woodblock roll
baroque receipt
grimoire tier
manuscript evidence
pastel donut
cutout stream
meadow survival
manga share
cyber mosaic
LIST
echo "→ $OUT"
# 片尾字幕用的三种版式（promo/long/strip.py 接成一条）；算账那一段浮出来的署名卡
R=promo/.build/long/roll
mkdir -p $R
for v in production crew end; do
  [ -f $R/$v.png ] || node packages/cli/bin/creditia.mjs export --credits $C --piece roll --style cinema --variant $v --lang en --scale 2 --out $R/$v.png >/dev/null 2>&1
done
[ -f promo/.build/long/card.png ] || node packages/cli/bin/creditia.mjs export --credits $C --piece share --style minimal-dark --lang en --scale 2 --out promo/.build/long/card.png >/dev/null 2>&1
echo "→ $R"

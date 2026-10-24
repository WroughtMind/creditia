#!/usr/bin/env bash
# 短片用的作品图：同一张分享图（样例项目 catnap）的全部 26 种风格，2 倍尺寸
set -euo pipefail
cd "$(dirname "$0")/../.."
OUT=promo/.build/short/prints
mkdir -p $OUT
for s in minimal-light minimal-dark glass film cinema editorial broadsheet playbill gala mono poster terminal crt neon arcade ink ukiyoe woodblock baroque grimoire manuscript pastel cutout meadow manga cyber; do
  [ -f $OUT/$s.png ] && continue
  node packages/cli/bin/creditia.mjs export --credits packages/render/fixtures/catnap.json --piece share --style $s --lang en --scale 2 --out $OUT/$s.png >/dev/null
done
echo "→ $OUT"

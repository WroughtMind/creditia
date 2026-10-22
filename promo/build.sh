#!/usr/bin/env bash
# 重新生成宣传片：作品图 → 画面 → 配乐 → 合成 → site/promo.mp4 + site/promo.jpg
# 需要：node 22+、python3（numpy、scipy、pillow）、playwright（PLAYWRIGHT_DIR）、ffmpeg（FFMPEG）
set -euo pipefail
cd "$(dirname "$0")/.."
FFMPEG=${FFMPEG:-ffmpeg}
FPS=${FPS:-60}
B=promo/.build
mkdir -p $B/pieces
C=packages/render/fixtures/catnap.json
CLI=packages/cli/bin/creditia.mjs
while read -r name piece style variant; do
  [ -f $B/pieces/$name.png ] && continue
  node $CLI export --credits $C --piece "$piece" --style "$style" ${variant:+--variant $variant} --lang en --scale 2 --out $B/pieces/$name.png
done < promo/pieces.txt
node promo/render.mjs events $B/events.json
python3 promo/score.py $B/events.json $B/score.wav
node promo/render.mjs video $B/picture.mp4 "$FPS"
# picture.mp4 是 CRF 16 的母版；放进仓库的是网页版（约 16 MB），响度拉到 -15 LUFS
"$FFMPEG" -hide_banner -loglevel error -y -i $B/score.wav -af "loudnorm=I=-15:TP=-1.2:LRA=11" -ar 48000 $B/score-norm.wav
"$FFMPEG" -hide_banner -loglevel error -y -threads 1 -i $B/picture.mp4 -an -c:v libx264 -preset medium -crf 25 -threads 1 -x264-params rc-lookahead=15 -pix_fmt yuv420p $B/web-v.mp4
"$FFMPEG" -hide_banner -loglevel error -y -i $B/web-v.mp4 -i $B/score-norm.wav -c:v copy -c:a aac -b:a 192k -movflags +faststart -shortest site/promo.mp4
node promo/render.mjs stills 23.6 $B/poster && python3 promo/poster.py $B/poster/t23.60.png site/promo.jpg
echo "→ site/promo.mp4"

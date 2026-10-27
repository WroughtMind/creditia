#!/usr/bin/env bash
# 长片《片尾字幕》：真数据 → 作品图、片尾字幕 → 贴图 → 时间表 → 配乐 → 画面（逐帧）→ 合成
#   promo/long/build.sh                   1920×1080 60fps → promo/.build/long/creditia-long.mp4
#   FPS=30 SCALE=0.5 promo/long/build.sh  快速预览
# 需要：node 22.18+、python3（numpy、scipy、pillow）、ffmpeg（FFMPEG）、playwright（PLAYWRIGHT_DIR，默认 promo/）
# 第一次：packages/render 和 packages/cli 里各 npm install 一次（CLI 导出作品图，片子里的字体也从那里取）
set -euo pipefail
cd "$(dirname "$0")/../.."
FFMPEG=${FFMPEG:-ffmpeg}
FPS=${FPS:-60}
SCALE=${SCALE:-1}
B=promo/.build/long
SUF="" # 长片只做横版
unset ASPECT
mkdir -p $B
(cd promo && npm install --no-audit --no-fund --silent)
node promo/long/prep.mjs $B/data.json
promo/long/prints.sh
python3 promo/long/strip.py $B/roll $B/tex
python3 promo/long/cardbars.py $B/card.png $B/tex/card.json
python3 promo/long/textures.py $B
node promo/long/render.mjs events $B/events.json
python3 promo/long/score.py $B/events.json $B/score.wav
# 分段渲染：每段一个新的浏览器，已经渲好的段跳过，断了可以接着跑；改了画面要先删掉 $B/seg$SUF
DUR=$(node -e "console.log(JSON.parse(require('fs').readFileSync('$B/events.json')).duration)")
SEG=6
S=$B/seg$SUF
mkdir -p $S
: > $S/list.txt
for i in $(seq 0 $(node -e "console.log(Math.ceil($DUR / $SEG) - 1)")); do
  from=$((i * SEG)); to=$(node -e "console.log(Math.min($DUR, ($i + 1) * $SEG))")
  out=$S/$(printf %03d "$i")-$FPS-$SCALE.mp4
  if [ ! -f "$out" ]; then
    node promo/long/render.mjs video "${out%.mp4}.part.mp4" "$FPS" "$from" "$to" "$SCALE"
    mv "${out%.mp4}.part.mp4" "$out"
  fi
  echo "file '$(basename "$out")'" >> $S/list.txt
done
# 母版各段是 CRF 12；成片：响度 -16 LUFS（安静的片子，不追响），CRF 18
"$FFMPEG" -hide_banner -loglevel error -y -i $B/score.wav -af "loudnorm=I=-16:TP=-1.5:LRA=14" -ar 48000 $B/score-norm.wav
"$FFMPEG" -hide_banner -loglevel error -y -f concat -safe 0 -i $S/list.txt -i $B/score-norm.wav \
  -map 0:v -map 1:a -c:v libx264 -preset slow -crf 18 -pix_fmt yuv420p -c:a aac -b:a 192k -movflags +faststart -shortest \
  $B/creditia-long$SUF.mp4
# 封面：看样台亮起来、署名卡在正中的那一刻
node promo/long/render.mjs stills 46.2 $B/poster$SUF "$SCALE"
"$FFMPEG" -hide_banner -loglevel error -y -i $B/poster$SUF/t46.20.png -q:v 2 $B/creditia-long$SUF.jpg
echo "→ $B/creditia-long$SUF.mp4"

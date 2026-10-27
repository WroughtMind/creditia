// 逐帧渲染长片《片尾字幕》：和短片同一个渲染器（promo/short/render.mjs），参数也一样
//   node promo/long/render.mjs stills 4,16,30 [out-dir] [scale]
//   node promo/long/render.mjs events [out.json]
//   node promo/long/render.mjs video out.mp4 [fps] [from] [to] [scale]
process.env.FILM = 'long';
await import('../short/render.mjs');

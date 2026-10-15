"""creditia 宣传片配乐与音效：全部由代码合成，卡 scene.html 的时间表。

    python3 promo/score.py promo/.build/events.json promo/.build/score.wav

120 BPM，D 小调。开场打字声与低频铺底 → 扫描上升音 → 重击 → 心跳脉冲与「叮」→
笔画汇聚的颗粒声 → 卡点鼓组配合作品快切 → 片尾字幕的钢琴和弦 → 标志与收尾和弦。
"""
import json
import sys

import numpy as np
from scipy import signal

SR = 48000
ev = json.load(open(sys.argv[1]))
T = ev["T"]
DUR = ev["duration"]
N = int(DUR * SR)
rng = np.random.default_rng(5)

# 总线：干声、混响送、鼓（被侧链的除外）、被侧链压缩的乐器
dry = np.zeros((N, 2))
verb = np.zeros((N, 2))
duck_bus = np.zeros((N, 2))


def note(n):
    """音名 → 频率，如 'D2' 'F#4' 'Bb3'"""
    names = {"C": 0, "D": 2, "E": 4, "F": 5, "G": 7, "A": 9, "B": 11}
    k = names[n[0]]
    i = 1
    while n[i] in "#b":
        k += 1 if n[i] == "#" else -1
        i += 1
    octave = int(n[i:])
    return 440.0 * 2 ** ((k + 12 * (octave + 1) - 69) / 12)


def tt(dur):
    return np.arange(int(dur * SR)) / SR


def place(bus, x, t0, gain=1.0, pan=0.0):
    """把单声道或立体声片段放进总线，pan -1..1（等功率）"""
    i0 = int(t0 * SR)
    if i0 >= N:
        return
    if x.ndim == 1:
        a = (pan + 1) * np.pi / 4
        x = np.stack([x * np.cos(a), x * np.sin(a)], axis=1)
    if i0 < 0:
        x = x[-i0:]
        i0 = 0
    n = min(len(x), N - i0)
    bus[i0:i0 + n] += x[:n] * gain


def env_adsr(n, a, d, s, r, sr=SR):
    a, d, r = int(a * sr), int(d * sr), int(r * sr)
    hold = max(0, n - a - d - r)
    e = np.concatenate([np.linspace(0, 1, a, endpoint=False), np.linspace(1, s, d, endpoint=False), np.full(hold, s), np.linspace(s, 0, r)])
    return np.pad(e, (0, max(0, n - len(e))))[:n]


def lp(x, f, order=2):
    sos = signal.butter(order, min(f, SR * 0.45), "low", fs=SR, output="sos")
    return signal.sosfilt(sos, x, axis=0)


def hp(x, f, order=2):
    sos = signal.butter(order, f, "high", fs=SR, output="sos")
    return signal.sosfilt(sos, x, axis=0)


def bp(x, lo, hi, order=2):
    sos = signal.butter(order, [lo, min(hi, SR * 0.45)], "band", fs=SR, output="sos")
    return signal.sosfilt(sos, x, axis=0)


def sweep_lp(x, f0, f1, curve=1.0, block=256, q=0.9):
    """随时间变化截止频率的低通（分块换系数，状态延续）"""
    out = np.zeros_like(x)
    n = len(x)
    zi = None
    for i in range(0, n, block):
        k = (i / max(1, n - 1)) ** curve
        f = f0 * (f1 / f0) ** k
        b, a = signal.iirfilter(2, min(f, SR * 0.45), btype="low", ftype="butter", fs=SR)
        if zi is None:
            zi = signal.lfilter_zi(b, a) * 0
        seg, zi = signal.lfilter(b, a, x[i:i + block], zi=zi)
        out[i:i + block] = seg
    return out


def sweep_bp(x, f0, f1, curve=1.0, block=256, width=0.6):
    out = np.zeros_like(x)
    n = len(x)
    zi = None
    for i in range(0, n, block):
        k = (i / max(1, n - 1)) ** curve
        f = f0 * (f1 / f0) ** k
        lo, hi = f * (1 - width / 2), min(f * (1 + width / 2), SR * 0.45)
        b, a = signal.iirfilter(1, [lo, hi], btype="band", ftype="butter", fs=SR)
        if zi is None:
            zi = signal.lfilter_zi(b, a) * 0
        seg, zi = signal.lfilter(b, a, x[i:i + block], zi=zi)
        out[i:i + block] = seg
    return out


def saw(f, t, phase=0.0):
    return signal.sawtooth(2 * np.pi * f * t + phase)


def supersaw(f, dur, voices=7, detune=0.012):
    t = tt(dur)
    out = np.zeros((len(t), 2))
    for v in range(voices):
        d = 1 + detune * (v - (voices - 1) / 2) / ((voices - 1) / 2)
        x = saw(f * d, t, rng.uniform(0, 6.28))
        pan = (v / (voices - 1)) * 2 - 1
        out[:, 0] += x * np.cos((pan + 1) * np.pi / 4)
        out[:, 1] += x * np.sin((pan + 1) * np.pi / 4)
    return out / voices


def noise(dur):
    return rng.standard_normal(int(dur * SR))


def pluck(f, dur=0.6, bright=4000, decay=6.0):
    t = tt(dur)
    x = np.zeros_like(t)
    for h in range(1, 9):
        if f * h > SR / 2.2:
            break
        x += np.sin(2 * np.pi * f * h * t) * (1 / h) * np.exp(-t * decay * (1 + h * 0.35))
    x *= np.minimum(1, t / 0.002)
    return lp(x, bright)


def bell(f, dur=2.5, index=2.2, ratio=3.5, decay=2.2):
    t = tt(dur)
    mod = np.sin(2 * np.pi * f * ratio * t) * index * np.exp(-t * decay * 1.6)
    x = np.sin(2 * np.pi * f * t + mod) * np.exp(-t * decay)
    x += 0.25 * np.sin(2 * np.pi * f * 2.0 * t) * np.exp(-t * decay * 2)
    return x * np.minimum(1, t / 0.003)


def epiano(f, dur=3.0):
    t = tt(dur)
    mod = np.sin(2 * np.pi * f * t) * 1.3 * np.exp(-t * 3.0)
    x = np.sin(2 * np.pi * f * t + mod) * np.exp(-t * 1.1)
    x += 0.18 * np.sin(2 * np.pi * f * 4.0 * t) * np.exp(-t * 6)  # 敲击的「叮」
    return x * np.minimum(1, t / 0.004)


def kick(dur=0.5, f0=150, f1=42, punch=1.0):
    t = tt(dur)
    f = f1 + (f0 - f1) * np.exp(-t * 28)
    ph = 2 * np.pi * np.cumsum(f) / SR
    x = np.sin(ph) * np.exp(-t * 6.5)
    click = hp(noise(dur), 2000) * np.exp(-t * 300) * 0.25
    return np.tanh((x + click * punch) * 1.6) * 0.8


def snare(dur=0.35):
    t = tt(dur)
    body = np.sin(2 * np.pi * 190 * t) * np.exp(-t * 22) * 0.5
    nz = bp(noise(dur), 1500, 9000) * np.exp(-t * 14)
    return body + nz * 0.7


def clap(dur=0.4):
    t = tt(dur)
    e = np.zeros_like(t)
    for off in (0, 0.011, 0.022):
        e += np.exp(-np.maximum(0, t - off) * 60) * (t >= off)
    e += np.exp(-np.maximum(0, t - 0.03) * 12) * (t >= 0.03) * 0.6
    return bp(noise(dur), 900, 6000) * e * 0.6


def hat(dur=0.08, open_=False):
    t = tt(dur)
    x = hp(noise(dur), 7500, 4)
    return x * np.exp(-t * (18 if open_ else 70))


def tick(f=4200, dur=0.03):
    t = tt(dur)
    return (np.sin(2 * np.pi * f * t) * 0.6 + bp(noise(dur), f * 0.7, f * 1.6) * 0.8) * np.exp(-t * 260)


def whoosh(dur, f0, f1, curve=1.0, peak=0.6):
    x = sweep_bp(noise(dur), f0, f1, curve=curve, width=0.9)
    t = np.linspace(0, 1, len(x))
    e = np.where(t < peak, (t / peak) ** 2, ((1 - t) / (1 - peak)) ** 1.5)
    return x * e


def impact(dur=4.0, root="D1"):
    """重击：下潜的超低频 + 噪声爆 + 铜管感的低音和弦（braam）"""
    t = tt(dur)
    f = 30 + 55 * np.exp(-t * 5)
    sub = np.sin(2 * np.pi * np.cumsum(f) / SR) * np.exp(-t * 1.3)
    burst = lp(noise(dur), 3500) * np.exp(-t * 9) * 0.7
    br = np.zeros((len(t), 2))
    for n_, g_ in ((root, 1.0), (root[:-1] + str(int(root[-1]) + 1), 0.8), ("A" + str(int(root[-1]) + 1), 0.5), (root[:-1] + str(int(root[-1]) + 2), 0.35)):
        br += supersaw(note(n_), dur, voices=5, detune=0.008) * g_
    e = np.exp(-t * 0.9) * np.minimum(1, t / 0.015)
    for c in range(2):
        br[:, c] = sweep_lp(br[:, c] * e, 2600, 160, curve=0.5)
    br = np.tanh(br * 2.2) * 0.55
    mono = sub * 0.9 + burst
    return br + mono[:, None]


def reverb_ir(dur=3.2, damp=5000):
    t = tt(dur)
    ir = np.stack([noise(dur), noise(dur)], axis=1) * np.exp(-t * (6.9 / dur))[:, None]
    ir = lp(ir, damp)
    ir[: int(0.012 * SR)] *= np.linspace(0, 1, int(0.012 * SR))[:, None]
    return ir / np.sqrt(np.sum(ir ** 2) / 2)


BEAT = 0.5

# ═════════ 1. 开场：铺底 + 打字 ═════════
# 低频铺底：D 小调和弦，滤波慢慢打开，在重击前收住
pad_end = T["hit"]
for n_, g_ in (("D2", 0.5), ("A2", 0.35), ("D3", 0.3), ("F3", 0.22), ("A3", 0.16)):
    x = supersaw(note(n_), pad_end + 0.5, voices=5, detune=0.006)
    for c in range(2):
        x[:, c] = sweep_lp(x[:, c], 140, 1900, curve=1.6)
    e = np.clip(tt(pad_end + 0.5) / 5.0, 0, 1) ** 1.5
    e *= np.clip((pad_end + 0.02 - tt(pad_end + 0.5)) / 0.25, 0, 1)
    place(dry, x * e[:, None], 0.0, 0.16 * g_)
    place(verb, x * e[:, None], 0.0, 0.10 * g_)
# 超低频持续音
t_ = tt(pad_end)
sub = np.sin(2 * np.pi * note("D1") * t_) * np.clip(t_ / 6, 0, 1) * np.clip((pad_end - t_) / 0.2, 0, 1)
place(dry, sub, 0, 0.22)

# 光标闪烁两下的轻响
for t0 in (0.15, 0.6):
    place(dry, tick(2600, 0.02), t0, 0.05, 0.0)
# 每个字符一个机械键响，随机音高与声像
for t0 in ev["type"]:
    f = rng.uniform(3200, 5200)
    place(dry, tick(f), t0, rng.uniform(0.05, 0.09), rng.uniform(-0.25, 0.25))
    if rng.random() < 0.5:
        place(dry, lp(noise(0.02), 900) * np.exp(-tt(0.02) * 200), t0, 0.08)
# 其他行冒出来：更轻、更散
for t0 in ev["lines"]:
    place(dry, tick(rng.uniform(5000, 8000), 0.02), t0, rng.uniform(0.012, 0.03), rng.uniform(-0.9, 0.9))
# 拉远时的颗粒：成千上万行同时写下
z0, z1 = T["zoomOut"]
for i in range(900):
    t0 = z0 - 0.2 + (z1 - z0 + 0.6) * (rng.random() ** 0.8)
    k = (t0 - z0) / (z1 - z0)
    place(dry, tick(rng.uniform(5000, 11000), 0.015), t0, 0.012 * (1 - abs(k - 0.4)), rng.uniform(-1, 1))
# 拉远的气流
w = whoosh(z1 - z0 + 0.6, 5000, 300, curve=0.7, peak=0.45)
place(dry, w, z0, 0.10)
place(verb, w, z0, 0.10)

# ═════════ 2. 扫描：上升音，一口吸气，然后重击 ═════════
s0, s1 = T["scan"]
dur = T["hit"] - s0 + 0.0
t_ = tt(dur)
riser_n = sweep_bp(noise(dur), 400, 9000, curve=1.6, width=0.8) * (t_ / dur) ** 2
place(dry, riser_n, s0, 0.16)
place(verb, riser_n, s0, 0.10)
# 音高上升的锯齿，带加速颤音
f = 110 * 2 ** (2.4 * (t_ / dur) ** 1.7)
ph = 2 * np.pi * np.cumsum(f) / SR
trem = 0.6 + 0.4 * np.sin(2 * np.pi * np.cumsum(2 + 16 * (t_ / dur) ** 2) / SR)
tone = lp(signal.sawtooth(ph) + signal.sawtooth(ph * 1.005), 3000) * trem * (t_ / dur) ** 2.2
place(dry, tone, s0, 0.05, -0.2)
place(verb, tone, s0, 0.07, 0.2)
# 光束的嗡鸣
hum = np.sin(2 * np.pi * 880 * tt(s1 - s0) + 0.6 * np.sin(2 * np.pi * 3 * tt(s1 - s0))) * np.sin(np.pi * tt(s1 - s0) / (s1 - s0)) ** 2
place(dry, hum, s0, 0.012)
# 重击
imp = impact(5.0, "D1")
place(dry, imp, T["hit"], 0.75)
place(verb, imp, T["hit"], 0.35)
place(dry, hat(1.6, True), T["hit"], 0.25)
place(verb, hat(2.0, True), T["hit"], 0.25)

# 图例逐个出现：拨弦
for i, n_ in enumerate(("D5", "F5", "A5", "C6", "D6")):
    t0 = T["legend"] + 0.25 + i * 0.12
    p = pluck(note(n_), 0.8, bright=5000)
    place(dry, p, t0, 0.07, -0.5 + i * 0.25)
    place(verb, p, t0, 0.08)

# ═════════ 3. 心跳脉冲与推近 ═════════
pulse_end = T["back"][0]
t0 = T["hit"] + 2.0
while t0 < pulse_end + 0.6:
    place(dry, kick(0.6, 110, 40, punch=0.2), t0, 0.32)
    t0 += 1.0
t0 = T["dive"][1]
while t0 < pulse_end:
    place(dry, hat(0.05), t0 + 0.25, 0.035, 0.3)
    t0 += 0.5
# 低音 drone：A 到 D
for (a, b, n_) in ((T["hit"] + 1.5, T["dive"][0] + 2, "D2"), (T["dive"][0] + 1.5, T["fly"] + 2.6, "D2")):
    x = supersaw(note(n_), b - a, voices=3, detune=0.004)
    x = lp(x, 500)
    e = env_adsr(len(x), 1.2, 0.1, 1.0, 1.2)
    place(dry, x * e[:, None], a, 0.10)
# 推近、拉回的气流
place(dry, whoosh(1.4, 300, 6000, curve=1.2, peak=0.75), T["dive"][0] - 0.2, 0.16)
place(verb, whoosh(1.4, 300, 6000, curve=1.2, peak=0.75), T["dive"][0] - 0.2, 0.08)
place(dry, whoosh(1.0, 6000, 400, curve=0.8, peak=0.5), T["back"][0] - 0.15, 0.14)
# 每一行的「叮」：FM 钟 + 一点超低频
for i, t0 in enumerate(T["blame"]):
    n_ = ("A5", "C6", "D6")[i]
    b_ = bell(note(n_), 3.0)
    place(dry, b_, t0, 0.10, (-0.3, 0.0, 0.3)[i])
    place(verb, b_, t0, 0.18)
    place(dry, kick(0.5, 90, 35, punch=0.5), t0, 0.35)
    place(dry, tick(6000), t0, 0.06)
    p = pluck(note(n_) / 4, 1.2, bright=1200, decay=2.5)
    place(dry, p, t0, 0.12)

# ═════════ 4. 笔画汇成一条：颗粒 + 计数 ═════════
f0 = T["fly"]
for i in range(1400):
    u = rng.random()
    t0 = f0 + u * 1.9
    fr = 1500 + 6000 * u + rng.uniform(-400, 400)
    place(dry, tick(fr, 0.012), t0, 0.010 + 0.012 * np.sin(np.pi * u), rng.uniform(-1, 1))
sw = whoosh(2.2, 800, 7000, curve=1.0, peak=0.6)
place(dry, sw, f0, 0.10)
place(verb, sw, f0, 0.08)
# 数字滚动：先快后慢的齿轮声
ta = T["tally"]
k = 0.0
while k < 1.0:
    t0 = ta + 1.1 * (1 - (1 - k) ** (1 / 3))  # 与画面 eo3 计数对齐
    place(dry, tick(2400 + 1600 * k, 0.02), t0, 0.04, rng.uniform(-0.2, 0.2))
    k += 0.025
b_ = bell(note("A5"), 2.0, index=1.2)
place(dry, b_, ta + 1.1, 0.06)
place(verb, b_, ta + 1.1, 0.10)
# 进入快切前的上升
rd = T["montage"] - (ta + 0.4)
x = sweep_bp(noise(rd), 600, 12000, curve=2.0, width=0.7) * (tt(rd) / rd) ** 3
place(dry, x, ta + 0.4, 0.22)
# 反向镲
rev = (hp(noise(1.2), 4000) * np.exp(-tt(1.2) * 3))[::-1]
place(dry, rev, T["montage"] - 1.2, 0.16)

# ═════════ 5. 快切：鼓组 + 贝斯 + 琶音 ═════════
m0, mf, mw = T["montage"], T["fast"], T["wall"]
chords = [("D", ["D", "F", "A"]), ("Bb", ["Bb", "D", "F"]), ("F", ["F", "A", "C"]), ("C", ["C", "E", "G"])]
# 鼓
nbeats = int((mw - m0) / BEAT)
for b in range(nbeats):
    t0 = m0 + b * BEAT
    fast = t0 >= mf
    place(dry, kick(0.5, 160, 45), t0, 0.7)
    if b % 2 == 1:
        place(dry, clap(), t0, 0.35, 0.05)
        place(dry, snare(), t0, 0.22)
        place(verb, clap(), t0, 0.18)
    for s in range(4):
        place(dry, hat(0.06 if s % 2 else 0.04), t0 + s * BEAT / 4, 0.05 if s == 2 else 0.028, 0.35)
    if not fast and b % 4 == 3:
        place(dry, hat(0.25, True), t0 + BEAT / 2, 0.06, -0.3)
# 快切段：军鼓滚奏加速
t0 = mf
i = 0
while t0 < mw - 0.02:
    k = (t0 - mf) / (mw - mf)
    place(dry, snare(0.2), t0, 0.08 + 0.18 * k, rng.uniform(-0.2, 0.2))
    t0 += BEAT / 4 if k < 0.5 else BEAT / 8
x = sweep_bp(noise(mw - mf), 500, 10000, curve=1.5, width=0.8) * np.linspace(0, 1, int((mw - mf) * SR)) ** 2
place(dry, x, mf, 0.2)
# 每一刀的快门声
for c in ev["cuts"]:
    place(dry, tick(rng.uniform(1800, 2600), 0.03), c, 0.05, rng.uniform(-0.4, 0.4))
    place(dry, whoosh(0.18, 3000, 9000, peak=0.3), c - 0.06, 0.03, rng.uniform(-0.5, 0.5))
# 贝斯：八分音符，跟和弦
for b in range(nbeats * 2):
    t0 = m0 + b * BEAT / 2
    bar = int((t0 - m0) / 2.0)
    root = chords[bar % 4][0] if t0 < mf else ("D" if (t0 - mf) < 1.0 else "Bb")
    f = note(root + "1") * (2 if b % 2 else 1)
    d = BEAT / 2 * 0.9
    x = lp(saw(f, tt(d)) + 0.5 * np.sin(2 * np.pi * f * tt(d)), 900) * env_adsr(int(d * SR), 0.003, 0.08, 0.6, 0.05)
    place(duck_bus, x, t0, 0.22)
# 琶音：十六分音符
for s in range(nbeats * 4):
    t0 = m0 + s * BEAT / 4
    bar = int((t0 - m0) / 2.0)
    ch = chords[bar % 4][1] if t0 < mf else (["D", "F", "A"] if (t0 - mf) < 1.0 else ["Bb", "D", "F"])
    seq = [ch[0] + "4", ch[1] + "4", ch[2] + "4", ch[0] + "5", ch[2] + "4", ch[1] + "4", ch[0] + "5", ch[1] + "5"]
    n_ = seq[s % 8]
    p = pluck(note(n_), 0.35, bright=2500 + 2500 * ((t0 - m0) / (mw - m0)), decay=9)
    place(duck_bus, p, t0, 0.07, 0.4 if s % 2 else -0.4)
    place(verb, p, t0, 0.03)
# 和声铺底
for bar in range(int((mf - m0) / 2.0)):
    rootn, ch = chords[bar % 4]
    for j, n_ in enumerate(ch):
        x = supersaw(note(n_ + "3"), 2.1, voices=5, detune=0.01)
        x = lp(x, 1800) * env_adsr(int(2.1 * SR), 0.05, 0.3, 0.8, 0.3)[:, None]
        place(duck_bus, x, m0 + bar * 2.0, 0.05)
        place(verb, x, m0 + bar * 2.0, 0.03)
# 撞墙：重击 + 长和弦
imp = impact(4.0, "D1")
place(dry, imp, mw, 0.6)
place(verb, imp, mw, 0.35)
place(dry, hat(2.5, True), mw, 0.22)
place(verb, hat(3.0, True), mw, 0.3)
for n_ in ("D3", "A3", "D4", "F4", "A4"):
    x = supersaw(note(n_), 3.4, voices=5, detune=0.008)
    x = lp(x, 2400) * env_adsr(int(3.4 * SR), 0.02, 0.6, 0.5, 2.0)[:, None]
    place(dry, x, mw, 0.05)
    place(verb, x, mw, 0.07)
# 过渡到片尾字幕：反向长音
rev = (lp(noise(1.6), 3000) * np.exp(-tt(1.6) * 2.5))[::-1]
place(verb, rev, T["credits"][0] - 1.6, 0.12)

# ═════════ 6. 片尾字幕：电钢琴和弦 + 铺底 ═════════
c0, c1, c2 = T["credits"]
prog_ = [
    (c0, ["Bb2", "F3", "A3", "D4", "F4"]),        # Bbmaj7
    (c1, ["F2", "C3", "E3", "A3", "C4"]),         # Fmaj7
    (c2, ["G2", "D3", "F3", "Bb3", "A4"]),        # Gm9
]
for i, (t0, ns) in enumerate(prog_):
    for j, n_ in enumerate(ns):
        x = epiano(note(n_), 3.4)
        place(dry, x, t0 + j * 0.035, 0.06, -0.3 + j * 0.15)
        place(verb, x, t0 + j * 0.035, 0.08)
    end = prog_[i + 1][0] if i + 1 < len(prog_) else T["logo"]
    d = end - t0 + 0.6
    for n_ in ns[:3]:
        x = supersaw(note(n_), d, voices=5, detune=0.006)
        x = lp(x, 900) * env_adsr(int(d * SR), 0.8, 0.2, 0.9, 0.8)[:, None]
        place(dry, x, t0, 0.04)
        place(verb, x, t0, 0.04)
# 一点旋律：高音钟
for t0, n_ in ((c0 + 1.0, "D6"), (c0 + 1.5, "C6"), (c1 + 1.0, "A5"), (c1 + 1.5, "C6"), (c2 + 1.0, "D6"), (c2 + 1.5, "F6")):
    b_ = bell(note(n_), 2.0, index=1.0, decay=2.5)
    place(dry, b_, t0, 0.025, 0.2)
    place(verb, b_, t0, 0.06)
# 极轻的脉冲让时间往前走
t0 = c0
while t0 < T["logo"] - 0.6:
    place(dry, kick(0.5, 80, 38, punch=0.0), t0, 0.12)
    t0 += 1.0

# ═════════ 7. 标志：六条色带 + 星芒 + 收尾和弦 ═════════
L0 = T["logo"]
for i, n_ in enumerate(("D5", "F5", "A5", "C6", "D6", "F6")):
    t0 = L0 + i * 0.1 + 0.05
    p = pluck(note(n_), 0.9, bright=6000, decay=5)
    place(dry, p, t0, 0.07, -0.5 if i % 2 == 0 else 0.5)
    place(verb, p, t0, 0.07)
    place(dry, whoosh(0.25, 2000, 8000, peak=0.4), t0 - 0.1, 0.03, -0.6 if i % 2 == 0 else 0.6)
hitT = L0 + 1.0
# 星芒：一串极快的高音
for i, n_ in enumerate(("A6", "D7", "E7", "F#7", "A7", "D8")):
    b_ = bell(note(n_), 1.6, index=0.8, decay=3.5)
    place(dry, b_, hitT + i * 0.03, 0.02, rng.uniform(-0.6, 0.6))
    place(verb, b_, hitT + i * 0.03, 0.05)
# 收尾：D 大调加九，温暖地打开
imp = impact(6.0, "D1")
place(dry, imp, hitT, 0.5)
place(verb, imp, hitT, 0.35)
fin_d = DUR - hitT
for n_, g_ in (("D2", 1.0), ("A2", 0.8), ("D3", 0.7), ("F#3", 0.55), ("A3", 0.5), ("E4", 0.4), ("F#4", 0.3), ("A4", 0.25)):
    x = supersaw(note(n_), fin_d, voices=7, detune=0.007)
    for c in range(2):
        x[:, c] = sweep_lp(x[:, c], 3000, 600, curve=0.7)
    e = np.exp(-tt(fin_d) * 0.35) * np.minimum(1, tt(fin_d) / 0.05)
    place(dry, x * e[:, None], hitT, 0.05 * g_)
    place(verb, x * e[:, None], hitT, 0.06 * g_)
for j, n_ in enumerate(("D3", "A3", "F#4", "A4", "E5")):
    x = epiano(note(n_), 5.0)
    place(dry, x, hitT + j * 0.04, 0.06)
    place(verb, x, hitT + j * 0.04, 0.10)
t_ = tt(fin_d)
place(dry, np.sin(2 * np.pi * note("D1") * t_) * np.exp(-t_ * 0.5) * np.minimum(1, t_ / 0.02), hitT, 0.3)
# 网址出现时一声轻响
b_ = bell(note("A5"), 2.0, index=0.6, decay=3)
place(dry, b_, L0 + 2.8, 0.025)
place(verb, b_, L0 + 2.8, 0.05)

# ═════════ 混音 ═════════
# 侧链：鼓点把贝斯/琶音/铺底压下去
side = np.ones(N)
for b in range(nbeats):
    i0 = int((m0 + b * BEAT) * SR)
    n = int(0.3 * SR)
    curve = 1 - 0.65 * np.exp(-np.arange(n) / SR * 14)
    side[i0:i0 + n] = np.minimum(side[i0:i0 + n], curve[: len(side[i0:i0 + n])])
mix = dry + duck_bus * side[:, None]

ir = reverb_ir(3.4, 5500)
wet = np.stack([signal.fftconvolve(verb[:, c], ir[:, c])[:N] for c in range(2)], axis=1)
wet = hp(wet, 180)
mix = mix + wet * 0.55

# 母带：轻微低切、柔和限幅、响度
mix = hp(mix, 25)
mix *= 0.98 / np.max(np.abs(mix)) * 1.6
mix = np.tanh(mix) / np.tanh(1.6)
mix *= 10 ** (-1.0 / 20) / np.max(np.abs(mix))
# 首尾淡入淡出
fade_in = int(0.05 * SR)
mix[:fade_in] *= np.linspace(0, 1, fade_in)[:, None]
fo = int(1.2 * SR)
mix[-fo:] *= np.linspace(1, 0, fo)[:, None] ** 1.5

from scipy.io import wavfile

wavfile.write(sys.argv[2], SR, (mix * 32767).astype(np.int16))
rms = 20 * np.log10(np.sqrt(np.mean(mix ** 2)))
print(f"score → {sys.argv[2]}  {DUR}s  rms {rms:.1f} dBFS")

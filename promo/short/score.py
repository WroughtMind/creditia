"""creditia 短片《脚注》配乐：全部由代码合成，跟着 short/main.js 的时间表（events.json）走。

    python3 promo/short/score.py promo/.build/short/events.json promo/.build/short/score.wav

D 大调，没有鼓，不卡拍点。乐器和长片同一套：毛毡钢琴、弦乐铺底、玻璃泛音、颗粒、合成的大厅混响。
每个工具一个音（五声音阶里的一个音），画面里冒出谁的星号，就响谁的音；整个仓库亮起来时，这些音变成细雨。
"""
import json
import sys

import numpy as np
from scipy import signal

SR = 48000
ev = json.load(open(sys.argv[1]))
T = ev["T"]
DUR = ev["duration"] + 1.0
N = int(DUR * SR)
rng = np.random.default_rng(11)

import os
STEM_OUT = os.environ.get("SCORE_STEMS")
dry = np.zeros((N, 2))
wet = np.zeros((N, 2))  # 送混响


def midi(n):
    names = {"C": 0, "D": 2, "E": 4, "F": 5, "G": 7, "A": 9, "B": 11}
    k, i = names[n[0]], 1
    while n[i] in "#b":
        k += 1 if n[i] == "#" else -1
        i += 1
    return k + 12 * (int(n[i:]) + 1)


def hz(m):
    return 440.0 * 2 ** ((m - 69) / 12)


STEMS = {}
STEM = ["misc"]


def place(x, t0, gain=1.0, pan=0.0, send=0.5):
    i0 = int(round(t0 * SR))
    if x.ndim == 1:
        a = (pan + 1) * np.pi / 4
        x = np.stack([x * np.cos(a), x * np.sin(a)], axis=1)
    if i0 < 0:
        x, i0 = x[-i0:], 0
    n = min(len(x), N - i0)
    if n <= 0:
        return
    dry[i0:i0 + n] += x[:n] * gain * (1 - send * 0.5)
    wet[i0:i0 + n] += x[:n] * gain * send
    if STEM_OUT:
        b = STEMS.setdefault(STEM[0], np.zeros(N))
        b[i0:i0 + n] += x[:n].mean(axis=1) * gain


def lp1(x, fc):
    a = np.exp(-2 * np.pi * fc / SR)
    return signal.lfilter([1 - a], [1, -a], x, axis=0)


def bp(x, lo, hi, order=2):
    sos = signal.butter(order, [lo, min(hi, SR * 0.45)], "band", fs=SR, output="sos")
    return signal.sosfilt(sos, x, axis=0)


def lpf(x, f, order=2):
    sos = signal.butter(order, min(f, SR * 0.45), "low", fs=SR, output="sos")
    return signal.sosfilt(sos, x, axis=0)


# ───────── 毛毡钢琴 ─────────
def piano(m, vel=0.5, dur=None, felt=0.7):
    f0 = hz(m)
    t60 = float(np.clip(13 * (110 / f0) ** 0.62, 1.6, 16))
    L = dur if dur else min(t60 * 0.9, 9.0)
    n = int(L * SR)
    t = np.arange(n) / SR
    B = 7e-5 * 2 ** ((m - 40) / 12 * 1.15)
    out = np.zeros(n)
    hammer = 0.12 + 0.02 * (1 - vel)
    k = 1
    while True:
        fk = k * f0 * np.sqrt(1 + B * k * k)
        if fk > min(SR * 0.42, 9000 + 6000 * vel) or k > 40:
            break
        amp = abs(np.sin(np.pi * k * hammer)) / k ** (1.15 + 1.1 * (1 - vel) + 0.6 * felt)
        tau = t60 / 6.9 / (1 + (fk / 1800) ** 1.4)
        env = 0.72 * np.exp(-t / (tau * 0.22)) + 0.28 * np.exp(-t / tau)
        # 三根弦，各差零点几音分：慢慢的拍频
        det = np.array([-0.9, 0.15, 1.1]) * (0.6 + rng.random() * 0.5)
        part = np.zeros(n)
        for d in det:
            part += np.sin(2 * np.pi * fk * 2 ** (d / 1200) * t + rng.random() * 6.28)
        out += amp * env * part / 3
        k += 1
    # 琴槌：一小团被毛毡闷住的噪声 + 一下很短的低频
    hn = int(0.03 * SR)
    noise = rng.standard_normal(hn) * np.exp(-np.arange(hn) / SR / 0.006)
    noise = lp1(noise, 1200 + 2500 * vel)
    out[:hn] += noise * 0.05 * vel
    thump = np.sin(2 * np.pi * min(f0, 90) * t[:hn * 2]) * np.exp(-t[:hn * 2] / 0.012) * 0.06 * vel
    out[:hn * 2] += thump
    # 毛毡：整体再柔一点
    out = lp1(out, 1400 + 5200 * vel * (1 - felt * 0.6))
    a = int(0.002 * SR)
    out[:a] *= np.linspace(0, 1, a)
    if dur:
        r = int(0.35 * SR)
        out[-r:] *= np.linspace(1, 0, r) ** 2
    return out * (0.25 + 0.75 * vel)


def pnote(name, t0, vel=0.45, pan=0.0, dur=None, send=0.55, gain=0.5):
    STEM[0] = "piano"
    m = midi(name) if isinstance(name, str) else name
    pan = pan if pan else float(np.clip((m - 62) / 30, -0.6, 0.6))
    place(piano(m, vel, dur), t0, gain, pan, send)


def pchord(names, t0, vel=0.4, spread=0.025, **kw):
    for i, nm in enumerate(names):
        pnote(nm, t0 + i * spread + rng.random() * 0.008, vel * (0.9 + rng.random() * 0.2), **kw)


# ───────── 弦乐铺底 ─────────
def strings(names, t0, t1, gain=0.12, attack=2.2, release=3.0, bright=0.5, pan=0.0):
    STEM[0] = "strings"
    L = t1 - t0 + release
    n = int(L * SR)
    t = np.arange(n) / SR
    out = np.zeros((n, 2))
    for nm in names:
        f0 = hz(midi(nm))
        for v in range(4):  # 合奏：四件乐器，各自失谐、各自颤音
            det = 2 ** ((rng.random() - 0.5) * 14 / 1200)
            vib = 1 + 0.0028 * np.sin(2 * np.pi * (4.6 + rng.random() * 0.8) * t + rng.random() * 6) * np.clip((t - 0.8) / 1.5, 0, 1)
            ph = 2 * np.pi * np.cumsum(f0 * det * vib) / SR
            sig = np.zeros(n)
            for k in range(1, 14):
                if f0 * k > 7000:
                    break
                # 每个泛音的音量各自缓慢起伏：弓在弦上的不均匀
                wob = 1 + 0.25 * np.sin(2 * np.pi * (0.13 + rng.random() * 0.3) * t + rng.random() * 6)
                sig += np.sin(k * ph + rng.random() * 6) / k ** (1.35 - 0.4 * bright) * wob
            p = (v / 3 - 0.5) * 0.9 + pan
            a = (np.clip(p, -1, 1) + 1) * np.pi / 4
            out[:, 0] += sig * np.cos(a)
            out[:, 1] += sig * np.sin(a)
    env = np.clip(t / attack, 0, 1) ** 1.6
    rel = np.clip((L - t) / release, 0, 1) ** 1.3
    out *= (env * rel)[:, None]
    out = lpf(out, 1600 + 3400 * bright)
    place(out, t0, gain / len(names) * 0.18, 0.0, 0.7)  # 铺底：比钢琴低一截


def drone(name, t0, t1, gain=0.1, attack=4.0, release=4.0):
    STEM[0] = "drone"
    L = t1 - t0 + release
    n = int(L * SR)
    t = np.arange(n) / SR
    f = hz(midi(name))
    s = np.sin(2 * np.pi * f * t) + 0.25 * np.sin(2 * np.pi * 2 * f * t + 1) + 0.08 * np.sin(2 * np.pi * 3 * f * t + 2)
    s *= 1 + 0.08 * np.sin(2 * np.pi * 0.11 * t)
    env = np.clip(t / attack, 0, 1) ** 2 * np.clip((L - t) / release, 0, 1) ** 1.5
    place(s * env, t0, gain * 0.3, 0.0, 0.25)


# ───────── 玻璃泛音 ─────────
def glass(name, t0, gain=0.06, pan=0.0, decay=2.4, send=0.8):
    STEM[0] = "glass"
    f = hz(midi(name)) if isinstance(name, str) else name
    n = int((decay * 1.6) * SR)
    t = np.arange(n) / SR
    s = np.zeros(n)
    for r, a, d in [(1, 1, 1), (2.756, 0.32, 0.55), (5.404, 0.12, 0.32), (8.933, 0.05, 0.2)]:
        s += a * np.sin(2 * np.pi * f * r * t + rng.random() * 6) * np.exp(-t / (decay * d / 2.3))
    att = int(0.004 * SR)
    s[:att] *= np.linspace(0, 1, att)
    place(s, t0, gain * 0.8, pan, send)


# ───────── 颗粒（细雨一样的写入声）─────────
def grains(t0, t1, density_fn, gain=0.02):
    STEM[0] = "texture"
    t = t0
    while t < t1:
        d = density_fn(t)
        if d <= 0:
            t += 0.02
            continue
        t += rng.exponential(1 / d)
        n = int(0.012 * SR)
        x = rng.standard_normal(n) * np.exp(-np.arange(n) / SR / 0.0018)
        f = 2500 + rng.random() * 5000
        x = bp(x, f * 0.8, f * 1.25, 1)
        place(x, t, gain * (0.4 + rng.random()), rng.random() * 1.6 - 0.8, 0.35)


def air(t0, t1, gain=0.012, fc=700):
    STEM[0] = "texture"
    n = int((t1 - t0) * SR)
    x = lpf(rng.standard_normal((n, 2)), fc, 2)
    tt = np.arange(n) / SR
    env = np.clip(tt / 3, 0, 1) * np.clip((t1 - t0 - tt) / 3, 0, 1)
    place(x * env[:, None], t0, gain, 0, 0.2)


def swell(t0, t1, gain=0.03, f0=300, f1=2400):
    STEM[0] = "texture"
    """很轻的气流：带通噪声，中心频率慢慢升高（不是那种冲向重击的上升音）"""
    n = int((t1 - t0) * SR)
    x = rng.standard_normal((n, 2))
    out = np.zeros_like(x)
    B = 2048
    for i in range(0, n, B):
        k = i / n
        fc = f0 * (f1 / f0) ** k
        seg = bp(x[i:i + B + 512], fc * 0.7, fc * 1.4, 1)[:B]
        out[i:i + len(seg)] = seg
    tt = np.arange(n) / SR
    env = np.sin(np.pi * np.clip(tt / (t1 - t0), 0, 1)) ** 2
    place(out * env[:, None], t0, gain, 0, 0.6)


# ═════════ 谱面 ═════════
END = ev["duration"]
# 五个工具各一个音：Claude Code、Codex、Cursor、Kimi、Grok（都在 D 大调五声音阶里）
VOICE = ["A4", "D5", "F#5", "E5", "B4"]
VOICE_HI = ["A5", "D6", "F#6", "E6", "B5"]

# I. 第一行：暗场里很轻的空气，光标写字时一串极轻的笔尖声（按 token 一小团一小团地冒）
air(0, 9, 0.008)
h0, h1 = T["hero"]
for j in range(16):
    k = 1 - (1 - (j + 0.5) / 16) ** (1 / 3)  # 和画面里 eo3 的吐字节奏对得上
    tk = h0 + k * (h1 - h0)
    n = int(0.02 * SR)
    x = bp(rng.standard_normal(n) * np.exp(-np.arange(n) / SR / 0.003), 3000, 9000, 1)
    place(x, tk, 0.012, 0.25, 0.4)

# 行尾的 *：Claude 的音，第一次响
hs = T["heroStar"]
pnote(VOICE[0], hs, 0.36, send=0.6)
glass(VOICE_HI[0], hs + 0.01, 0.03, 0.2, 2.8)
drone("D2", hs - 0.4, T["gather"][0] + 0.5, 0.045, attack=3.0)

# II. 拉远：同一个文件的其余行冒出脚注 → 整个仓库一圈圈亮起来，拨弦变成细雨
pops = ev["pops"]
file_pops = [p for p in pops if p[2] == 1 and p[0] > hs + 0.2]
for t0, a, _ in file_pops:
    pnote(VOICE[a], t0, 0.24 + rng.random() * 0.06, send=0.6, gain=0.4)
map_pops = [p for p in pops if p[2] == 0]
# 远处的成千上万颗：抽一部分做玻璃雨点，其余交给颗粒声（密度跟着画面里冒星号的速度走）
for i, (t0, a, _) in enumerate(map_pops):
    if rng.random() < 140 / max(1, len(map_pops)):
        glass(VOICE_HI[a], t0, 0.006 + rng.random() * 0.006, rng.random() * 1.6 - 0.8, 1.4)
hist, edges = np.histogram([p[0] for p in map_pops], bins=60, range=(T["file"], T["gather"][0] + 0.5))
rate = hist / np.diff(edges)
peak_rate = max(1.0, rate.max())
grains(T["file"], T["gather"][0] + 0.5, lambda t: 160 * float(np.interp(t, (edges[:-1] + edges[1:]) / 2, rate)) / peak_rate, 0.01)
strings(["D3", "A3", "E4", "F#4"], T["pull"][0], T["gather"][0] + 0.6, 0.12, attack=2.6, bright=0.4)
pnote("F#4", T["pull"][0] + 1.4, 0.26)
pnote("E5", T["pull"][0] + 3.0, 0.26)

# III. 所有的 * 飞向中心：慢慢涨上去，星号长实时落到一个舒展的和弦上（不是重击）
g0, g1 = T["gather"]
p0, p1 = T["petals"]
swell(g0 - 0.2, p1, 0.02, 260, 3000)
strings(["B2", "F#3", "D4", "E4"], g0, g0 + 1.4, 0.12, attack=1.0, bright=0.5)
strings(["G2", "D3", "B3", "F#4"], g0 + 1.2, p0 + 0.3, 0.13, attack=0.9, bright=0.6)
for i, nm in enumerate(["D4", "E4", "F#4", "A4", "B4", "D5", "E5"]):
    pnote(nm, g0 + 0.35 + i * (p0 - g0 - 0.5) / 7, 0.2 + i * 0.03, gain=0.4)
settle = p0 + 0.15
pchord(["D2", "A2", "F#3", "E4", "A4"], settle, 0.42, 0.035)
glass("A5", settle + 0.05, 0.03, -0.2, 3.0)
strings(["D3", "A3", "E4", "F#4", "A4"], settle - 0.1, T["fan"][1], 0.15, attack=1.4, bright=0.5)
drone("D1", settle, T["fan"][1] + 0.5, 0.07, attack=1.2)
# 瓣尖的名字一个个浮出：各自的音，很轻
for i in range(5):
    glass(VOICE_HI[i], T["labels"][0] + i * 0.11, 0.012, (i - 2) * 0.3, 2.2)

# IV. 一叠牌浮出、捻开：往上走的琶音；一道波抬起每张牌，每张一声玻璃音，音阶跟着波从左往右走
r0, _ = T["rise"]
f0, f1 = T["fan"]
for i, nm in enumerate(["A3", "D4", "E4", "F#4", "A4", "B4", "D5"]):
    pnote(nm, f0 + i * (f1 - f0) / 7, 0.22 + 0.02 * i, gain=0.38)
scale_up = ["D5", "E5", "F#5", "A5", "B5", "D6", "E6", "F#6", "A6"]
for t0, i in ev["ripple"]:
    nm = scale_up[int(round(i / (len(ev["ripple"]) - 1) * 8))]
    glass(nm, t0, 0.011, (i / 25 - 0.5) * 1.2, 1.8)
q0, q1 = T["ripple"]
strings(["B2", "F#3", "D4", "A4"], f1 - 0.3, q0 + 1.3, 0.13, attack=1.2, bright=0.5)
strings(["G2", "D3", "B3", "F#4", "A4"], q0 + 1.1, q1 + 0.3, 0.14, attack=1.2, bright=0.55)
drone("G1", q0 + 1.1, q1 + 0.6, 0.05, attack=1.5)
pnote("F#5", q0 + 0.4, 0.26)
pnote("E5", q0 + 1.6, 0.24)

# V. 合拢、收回星号：和声往回收（A 挂留），字标出来时落在主和弦上
c0, c1 = T["close"]
strings(["A2", "E3", "D4", "E4"], c0 - 0.1, T["deck"][1], 0.13, attack=1.0, bright=0.45)
for i, nm in enumerate(["A5", "F#5", "E5", "D5"]):
    pnote(nm, c0 + 0.1 + i * 0.3, 0.24 - i * 0.02, gain=0.38)
W0 = T["word"][0]
pchord(["D2", "A2", "F#3", "E4", "A4", "C#5"], W0, 0.4, 0.05)
glass("A6", T["end"][0] + 0.35, 0.02, 0.3, 3.2)
strings(["D3", "A3", "E4", "F#4", "A4"], W0 - 0.2, END - 0.6, 0.15, attack=1.8, release=3.5, bright=0.4)
drone("D1", W0 - 0.2, END - 1.0, 0.07, attack=1.6, release=2.5)
pnote("F#5", T["cmd"][0] + 0.2, 0.22)

# ───────── 大厅混响：合成的冲激响应，高频比低频衰减得快 ─────────
def hall_ir(t60=3.6, pre=0.028):
    n = int((t60 + 0.3) * SR)
    t = np.arange(n) / SR
    ir = np.zeros((n, 2))
    for lo, hi, scale in [(40, 400, 1.15), (400, 2000, 1.0), (2000, 6000, 0.7), (6000, 16000, 0.45)]:
        noise = bp(rng.standard_normal((n, 2)), lo, hi, 2)
        ir += noise * np.exp(-6.9 * t / (t60 * scale))[:, None]
    ir *= np.clip(t / 0.06, 0, 1)[:, None]
    # 早期反射
    for d, g in [(0.011, 0.5), (0.019, 0.35), (0.027, 0.3), (0.041, 0.22), (0.053, 0.18)]:
        i = int(d * SR)
        ir[i, 0] += g * (0.8 + 0.4 * rng.random())
        ir[i + int(0.0013 * SR), 1] += g * (0.8 + 0.4 * rng.random())
    p = int(pre * SR)
    ir = np.concatenate([np.zeros((p, 2)), ir])
    return ir / np.sqrt((ir ** 2).sum(axis=0, keepdims=True))


ir = hall_ir()
rev = np.stack([signal.fftconvolve(wet[:, c], ir[:, c])[:N] for c in range(2)], axis=1)
mix = dry + rev * 0.55
# 低切；低频搁架收一点（笔记本扬声器放不出来，只会把响度吃掉），高频搁架给钢琴和玻璃一点空气
def shelf(x, f0, gain_db, kind):
    a = 10 ** (gain_db / 40); w0 = 2 * np.pi * f0 / SR; al = np.sin(w0) / 2 * np.sqrt(2); c = np.cos(w0); k = 2 * np.sqrt(a) * al
    if kind == "low":
        b = [a * ((a + 1) - (a - 1) * c + k), 2 * a * ((a - 1) - (a + 1) * c), a * ((a + 1) - (a - 1) * c - k)]
        d = [(a + 1) + (a - 1) * c + k, -2 * ((a - 1) + (a + 1) * c), (a + 1) + (a - 1) * c - k]
    else:
        b = [a * ((a + 1) + (a - 1) * c + k), -2 * a * ((a - 1) + (a + 1) * c), a * ((a + 1) + (a - 1) * c - k)]
        d = [(a + 1) - (a - 1) * c + k, 2 * ((a - 1) - (a + 1) * c), (a + 1) - (a - 1) * c - k]
    return signal.lfilter(np.array(b) / d[0], np.array(d) / d[0], x, axis=0)


mix = signal.sosfilt(signal.butter(2, 32, "high", fs=SR, output="sos"), mix, axis=0)
mix = shelf(mix, 110, -3.5, "low")
mix = shelf(mix, 5000, 3.0, "high")
# 结尾淡出
tt = np.arange(N) / SR
mix *= np.clip((DUR - 0.3 - tt) / 2.4, 0, 1)[:, None] ** 1.5
mix *= np.clip(tt / 0.4, 0, 1)[:, None]
peak = np.abs(mix).max()
mix = np.tanh(mix / peak * 1.1) / np.tanh(1.1) * 0.9
from scipy.io import wavfile

wavfile.write(sys.argv[2], SR, (mix * 32767).astype(np.int16))
if STEM_OUT:
    np.savez_compressed(STEM_OUT, **{k: v.astype(np.float32) for k, v in STEMS.items()}, scale=np.float32(1.1 / peak))
print("score →", sys.argv[2], f"{DUR:.1f}s")

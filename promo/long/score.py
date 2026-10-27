"""creditia 长片《片尾字幕》配乐：全部由代码合成，跟着 long/main.js 的时间表（events.json）走。

    python3 promo/long/score.py promo/.build/long/events.json promo/.build/long/score.wav

D 大调，没有鼓，不卡拍点。乐器和短片同一套：毛毡钢琴、弦乐铺底、玻璃泛音、颗粒、合成的大厅混响。
终端里几乎没有声音，只有按键；延时摄影里是细雨一样的颗粒，跟着每天写的量起落，发版那天响一声；
证据一段每标一行响一个音（日志里对得上的最清楚，未确认的没有音高）；看样台最舒展；
片尾字幕滚起来时弦乐整个铺开，THE END 落在主和弦上，五颗菱形变成星号时五个工具的音各响一下，最后只剩一个音。
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
VOICE = ["A4", "D5", "F#5", "E5", "B4"]      # 五个工具各一个音（同短片）
VOICE_HI = ["A5", "D6", "F#6", "E6", "B5"]


def click(t0, gain=0.006, pan=0.0, f=4200):
    """按键：一小团闷住的高频噪声"""
    STEM[0] = "texture"
    n = int(0.018 * SR)
    x = rng.standard_normal(n) * np.exp(-np.arange(n) / SR / 0.0025)
    x = bp(x, f * 0.6, f * 1.6, 1)
    place(x, t0, gain, pan, 0.25)


# I. 终端：几乎无声。按键、回车，结果出来时一声很轻的玻璃，名片印出来时钢琴落一个和弦
air(0, 11, 0.007, 600)
for k in ev["keys"]:
    click(k, 0.005 + rng.random() * 0.003, 0.15 + rng.random() * 0.1)
for e in ev["enters"]:
    click(e, 0.009, 0.2, 2600)
first_out = min(ev["lines"])
glass("A5", T["term"]["out1"], 0.012, 0.2, 2.0)
pchord(["D3", "A3", "E4", "F#4"], T["term"]["out2"], 0.3, 0.04)
pnote("A4", T["term"]["out2"] + 1.1, 0.24)
# 钻进去：气流升起来，穿过进度条那一刻一个低音进来
d0, d1 = T["dive"]
swell(d0, d1 + 0.4, 0.016, 200, 2200)
drone("D2", d0 + 1.0, T["lapse"][1] + 1, 0.05, attack=2.5)
pnote("D3", d0 + 1.75, 0.34, send=0.7)
glass("D6", d0 + 1.75, 0.02, 0, 3.0)

# II. 141 天：颗粒声的密度跟着每天写的量；弦乐慢慢换和弦；发版那天响一声玻璃、一个低八度
L0, L1 = T["lapse"]
days = ev["days"]
vols = np.array([v for _, v in days], float)
times = np.array([t for t, _ in days], float)
peak = vols.max()
grains(L0, L1 + 0.6, lambda t: 180 * float(np.interp(t, times, vols, left=0, right=0)) / peak + 12, 0.009)
seg = (L1 - L0) / 4
for i, ch in enumerate([["D3", "A3", "E4", "F#4"], ["B2", "F#3", "D4", "A4"], ["G2", "D3", "B3", "F#4"], ["A2", "E3", "C#4", "E4"]]):
    strings(ch, L0 + i * seg - 0.4, L0 + (i + 1) * seg + 0.4, 0.12, attack=2.0, bright=0.4)
for i, nm in enumerate(["F#4", "A4", "D5", "C#5", "B4", "A4"]):
    pnote(nm, L0 + 0.8 + i * (L1 - L0 - 1.5) / 6, 0.24, gain=0.42)
for t0, tag in ev["versions"]:
    glass("A6" if tag.endswith("1.0") else "F#6" if tag.endswith("2.0") else "D7", t0, 0.03, 0.3, 3.0)
    pnote("D2", t0, 0.3, send=0.6, gain=0.4)

# III. 证据：安静下来，像凑近了看；每标一行响一个音
e0, e1 = T["ev"]
strings(["D3", "A3", "D4", "E4"], e0 - 1.0, e1, 0.09, attack=2.5, bright=0.3)
drone("D2", e0 - 1.0, e1 + 0.5, 0.04, attack=2.5)
for t0, c, kind in ev["tags"]:
    if kind == "verified":
        pnote(VOICE[c], t0, 0.32, gain=0.42)
        glass(VOICE_HI[c], t0 + 0.72, 0.012, 0.35, 1.8)          # 线落到日志里那一行
    elif kind == "declared":
        pnote(VOICE[c], t0, 0.22, gain=0.36)
        glass(VOICE_HI[c], t0 + 0.72, 0.007, 0.3, 1.4)
    elif kind == "inferred":
        pnote(VOICE[c], t0, 0.12, gain=0.3, dur=0.6)
    else:
        click(t0, 0.01, -0.2, 900)                                 # 未确认：没有音高，只有一下闷响

# IV. 算账：颜色往上流的时候往上走，落满时和弦铺开，卡片浮出来时一声玻璃
a0 = T["tally"][0]
landed = ev["tally"]
swell(a0, landed + 0.2, 0.02, 260, 3200)
for i, nm in enumerate(["D4", "E4", "F#4", "A4", "B4", "D5", "E5", "F#5"]):
    pnote(nm, a0 + 0.3 + i * (landed - a0 - 0.5) / 8, 0.2 + 0.025 * i, gain=0.4)
pchord(["D2", "A2", "F#3", "E4", "A4"], landed, 0.42, 0.035)
strings(["D3", "A3", "E4", "F#4", "A4"], landed - 0.1, T["table"][0] + 0.5, 0.15, attack=1.2, bright=0.5)
drone("D1", landed, T["table"][0] + 1.0, 0.06, attack=1.0)
glass("A5", landed + 1.3, 0.025, -0.2, 3.0)

# V. 看样台：最舒展的一段。灯亮起来时和声往上一抬，钢琴慢慢唱一句
b0, b1 = T["table"]
prog_ = [(["D2", "A2", "F#3", "A3", "E4"], ["F#5", "E5"]), (["F#2", "C#3", "A3", "E4"], ["C#5", "E5"]),
         (["G2", "D3", "B3", "F#4"], ["D5", "B4"]), (["A2", "E3", "C#4", "E4"], ["E5", "A4"]),
         (["B1", "F#2", "D3", "A3", "D4"], ["F#5", "D5"]), (["G1", "D2", "B2", "F#3", "A3"], ["E5", "B4", "A4"])]
step = (b1 - b0) / len(prog_)
for i, (ch, mel) in enumerate(prog_):
    t0 = b0 + i * step
    strings(ch[1:], t0 - 0.3, t0 + step + 0.4, 0.14, attack=1.8, bright=0.5)
    drone(ch[0], t0 - 0.3, t0 + step + 0.5, 0.05, attack=1.8, release=2.5)
    pchord(ch[:3], t0, 0.3, 0.05)
    for j, nm in enumerate(mel):
        pnote(nm, t0 + 0.6 + j * (step - 0.8) / len(mel), 0.34 - j * 0.03)
for k in range(8):
    glass(["D6", "A5", "F#6", "E6", "B5", "D6", "A5", "C#6"][k], b0 + 0.8 + k * 1.6 + rng.random() * 0.4, 0.007, rng.random() * 1.4 - 0.7, 2.0)

# VI. 片尾字幕：弦乐整个铺开，低音进来；THE END 落在主和弦上；菱形变星号时五个工具的音各响一下；最后只剩一个音
r0, r1 = T["roll"]
m0, m1 = T["mark"]
roll_prog = [["D2", "A2", "D3", "F#3", "A3", "E4"], ["B1", "F#2", "D3", "F#3", "B3"], ["G1", "D2", "B2", "D3", "G3", "D4"], ["A1", "E2", "C#3", "E3", "A3"]]
rs = (r1 - r0) / len(roll_prog)
for i, ch in enumerate(roll_prog):
    t0 = r0 + i * rs
    strings(ch[1:], t0 - 0.4, t0 + rs + 0.5, 0.2, attack=1.6, bright=0.55)
    drone(ch[0], t0 - 0.4, t0 + rs + 0.6, 0.08, attack=1.6, release=2.5)
    pchord(ch[1:4], t0, 0.34, 0.06)
for j, nm in enumerate(["A4", "D5", "F#5", "E5", "D5", "B4", "C#5", "E5"]):
    pnote(nm, r0 + 0.8 + j * (r1 - r0 - 1.2) / 8, 0.32)
pchord(["D2", "A2", "F#3", "A3", "D4", "F#4"], r1 - 0.2, 0.44, 0.05)
strings(["D3", "A3", "D4", "F#4", "A4"], r1 - 0.4, END - 0.5, 0.18, attack=1.2, release=4.0, bright=0.45)
drone("D1", r1 - 0.4, END - 1.0, 0.08, attack=1.2, release=3.0)
for i in range(5):
    glass(VOICE_HI[i], m0 + 0.35 + i * 0.22, 0.02, (i - 2) * 0.3, 2.8)
pchord(["D3", "A3", "E4", "A4", "C#5"], T["end"][0] + 0.4, 0.36, 0.05)
pnote("F#5", T["end"][1] + 0.6, 0.26)
pnote("D5", END - 3.6, 0.22, dur=3.4)

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

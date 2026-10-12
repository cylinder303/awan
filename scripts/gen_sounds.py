#!/usr/bin/env python3
"""Generate the bundled alert sounds (16-bit mono WAV, 44.1 kHz).

Every sound comes from nature or an everyday object: water, a door, birds,
a pebble, the sea. None imitates a musical instrument or plays a melody.
They are synthesized here so the app ships with no third-party audio, and a
fixed random seed makes every run produce the same files.

Needs numpy. Run from the repo root:  python3 scripts/gen_sounds.py
"""
import wave
from pathlib import Path

import numpy as np

RATE = 44100
ROOT = Path(__file__).resolve().parent.parent
OUT = ROOT / "src-tauri" / "sounds"
# Android plays notification sounds from the app's resources.
ANDROID_RAW = ROOT / "src-tauri" / "gen" / "android" / "app" / "src" / "main" / "res" / "raw"
TARGET_DB = -15.0  # loudest 400 ms, roughly matched across sounds
PEAK = 0.89  # -1 dBFS, headroom for the volume slider and resampling


def seconds(n):
    return np.arange(int(RATE * n)) / RATE


def band(x, low=None, high=None, order=2):
    """Zero-phase Butterworth-shaped band filter, applied in the frequency domain."""
    pad = int(RATE * 0.1)
    spec = np.fft.rfft(np.concatenate([x, np.zeros(pad)]))
    f = np.maximum(np.fft.rfftfreq(len(x) + pad, 1 / RATE), 1e-6)
    gain = np.ones_like(f)
    if low:
        gain /= np.sqrt(1 + (low / f) ** (2 * order))
    if high:
        gain /= np.sqrt(1 + (f / high) ** (2 * order))
    return np.fft.irfft(spec * gain, len(x) + pad)[: len(x)]


def place(out, start, sound):
    i = int(start * RATE)
    n = min(len(sound), len(out) - i)
    out[i : i + n] += sound[:n]


def reverb(x, rng, decay, wet, damp=5000.0):
    """A small room: exponentially decaying noise as the impulse response."""
    t = seconds(decay)
    ir = band(rng.standard_normal(len(t)) * np.exp(-6.9 * t / decay), high=damp, order=1)
    ir[: int(RATE * 0.01)] = 0  # first reflections arrive after ~10 ms
    ir /= np.sqrt(np.sum(ir**2))
    n = len(x) + len(ir)
    tail = np.fft.irfft(np.fft.rfft(x, n) * np.fft.rfft(ir, n), n)[: len(x)]
    return x + wet * tail


def impact(rng, modes, click, click_band, length, detune=0.0):
    """A struck object: a few damped resonances plus the sharp contact noise."""
    t = seconds(length)
    body = np.zeros(len(t))
    for freq, amp, decay in modes:
        f = freq * (1 + detune * rng.uniform(-1, 1))
        body += amp * np.exp(-t / decay) * np.sin(2 * np.pi * f * t + rng.uniform(0, 0.3))
    contact = band(rng.standard_normal(len(t)) * np.exp(-t / 0.0015), *click_band)
    return (1 - np.exp(-t / 0.0004)) * body + click * contact


# Water drops: a bubble's ring rises in pitch as it closes.
def drops(rng):
    out = np.zeros(int(RATE * 1.6))
    for start, f0, amp in ((0.0, 860, 1.0), (0.23, 1090, 0.75), (0.41, 730, 0.9), (0.86, 960, 0.6)):
        t = seconds(0.3)
        freq = f0 * (1 + 0.85 * (1 - np.exp(-t / 0.03)))
        ring = np.sin(2 * np.pi * np.cumsum(freq) / RATE) * (1 - np.exp(-t / 0.0008)) * np.exp(-t / 0.045)
        splash = band(rng.standard_normal(len(t)) * np.exp(-t / 0.0007), low=1800) * 0.12
        place(out, start, amp * (ring + splash))
    return reverb(out, rng, decay=0.5, wet=0.2, damp=6000)


# Three knocks on a wooden door.
DOOR = ((190, 1.0, 0.045), (330, 0.8, 0.035), (540, 0.6, 0.025), (890, 0.45, 0.018), (1400, 0.3, 0.011), (2300, 0.15, 0.006))


def knock(rng):
    out = np.zeros(int(RATE * 1.0))
    for start, amp in ((0.0, 1.0), (0.17, 0.9), (0.34, 1.0)):
        place(out, start, amp * impact(rng, DOOR, click=0.5, click_band=(900, 5000), length=0.25, detune=0.02))
    return reverb(out, rng, decay=0.45, wet=0.2, damp=4000)


# Two small birds: quick sweeping chirps, one bird near and one farther away.
def chirp(rng, path, length, amp, wobble=0.0):
    t = seconds(length)
    u = t / length
    freq = np.interp(u, np.linspace(0, 1, len(path)), path) * (1 + rng.uniform(-0.03, 0.03))
    freq *= 1 + wobble * np.sin(2 * np.pi * 38 * t)
    phase = 2 * np.pi * np.cumsum(freq) / RATE
    env = np.sin(np.pi * u) ** 1.5
    return amp * env * (np.sin(phase) + 0.12 * np.sin(2 * phase))


def birds(rng):
    out = np.zeros(int(RATE * 1.8))
    for start in (0.0, 0.13, 0.26):
        place(out, start, chirp(rng, (5000, 3500, 3800), 0.075, 1.0))
    for i in range(7):
        place(out, 0.56 + i * 0.05, chirp(rng, (4100, 3000), 0.035, 0.45 - i * 0.03, wobble=0.02))
    place(out, 1.06, chirp(rng, (3200, 4600), 0.11, 0.9))
    place(out, 1.22, chirp(rng, (4800, 3400), 0.07, 0.8))
    return reverb(out, rng, decay=0.35, wet=0.12, damp=7000)


# A pebble dropped on stone, bouncing to rest.
STONE = (
    (1700, 0.45, 0.009), (2650, 1.0, 0.007), (3300, 0.5, 0.006), (4100, 0.6, 0.005),
    (5900, 0.35, 0.004), (7600, 0.2, 0.003), (9800, 0.12, 0.002),
)


def pebble(rng):
    out = np.zeros(int(RATE * 1.3))
    start, gap, amp = 0.0, 0.28, 1.0
    for _ in range(9):
        place(out, start, amp * impact(rng, STONE, click=0.8, click_band=(1500, 12000), length=0.06, detune=0.08))
        start, gap, amp = start + gap, gap * 0.68, amp * 0.74
    return reverb(out, rng, decay=0.4, wet=0.15, damp=6000)


# A wave rolling in and washing out: low rumble, then the hiss of foam.
def swell(t, peak, rise, fall):
    up = 0.5 - 0.5 * np.cos(np.pi * np.clip((t - (peak - rise)) / rise, 0, 1))
    return np.where(t < peak, up, np.exp(-(t - peak) / fall))


def wave_(rng):
    t = seconds(3.0)
    noise = lambda: rng.standard_normal(len(t))  # noqa: E731
    out = band(noise(), 120, 700) * swell(t, 1.0, 1.0, 0.8)
    out += 0.7 * band(noise(), 700, 2500) * swell(t, 1.1, 0.9, 0.7)
    out += 0.35 * band(noise(), 2500, 9000) * swell(t, 1.3, 0.9, 0.9)
    # Bubbles popping in the foam.
    pops = (rng.random(len(t)) < 0.004 * swell(t, 1.4, 0.6, 0.6)) * rng.uniform(-1, 1, len(t))
    out += 0.25 * band(pops, 3000, 10000)
    return out


SOUNDS = {
    "drops": (drops, 0.0),
    "knock": (knock, 0.0),
    "birds": (birds, -3.0),  # high chirps sound louder than they measure
    "pebble": (pebble, 0.0),
    "wave": (wave_, 0.0),
}


def loudness_db(x):
    """Loudest 400 ms after a rough ear weighting (less bass, a bit more treble)."""
    k = band(x, low=100)
    k = k + 0.6 * band(k, low=1500)
    win = int(RATE * 0.4)
    energy = np.convolve(k**2, np.ones(win) / win, mode="valid") if len(k) > win else [np.mean(k**2)]
    return 10 * np.log10(max(np.max(energy), 1e-12))


def soft_limit(x, ceiling, knee=0.5):
    """Linear up to knee * ceiling, then curving smoothly towards the ceiling."""
    start = knee * ceiling
    over = np.maximum(np.abs(x) - start, 0)
    room = ceiling - start
    return np.where(over > 0, np.sign(x) * (start + room * np.tanh(over / room)), x)


def render(name, x, trim_db):
    x = x - np.mean(x)
    fade_in, fade_out = int(RATE * 0.002), int(RATE * 0.08)
    x[:fade_in] *= np.linspace(0, 1, fade_in)
    x[-fade_out:] *= np.linspace(1, 0, fade_out) ** 2
    gain = 10 ** ((TARGET_DB + trim_db - loudness_db(x)) / 20)
    # Sharp clicks would hit the ceiling long before they sound as loud as
    # the rest: let them go up to 4 dB further and round off their peaks.
    x = soft_limit(x * min(gain, 1.6 * PEAK / np.max(np.abs(x))), PEAK)
    for out in (OUT, ANDROID_RAW):
        out.mkdir(parents=True, exist_ok=True)
        with wave.open(str(out / f"{name}.wav"), "wb") as w:
            w.setnchannels(1)
            w.setsampwidth(2)
            w.setframerate(RATE)
            w.writeframes(np.round(x * 32767).astype("<i2").tobytes())
    print(f"wrote {name}.wav  {len(x) / RATE:.2f} s  peak {20 * np.log10(np.max(np.abs(x))):.1f} dBFS  loudness {loudness_db(x):.1f} dB")


if __name__ == "__main__":
    for i, (name, (make, trim_db)) in enumerate(SOUNDS.items()):
        render(name, make(np.random.default_rng(1000 + i)), trim_db)

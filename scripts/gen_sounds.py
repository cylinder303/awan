#!/usr/bin/env python3
"""Generate the bundled alert sounds (16-bit mono WAV, 44.1 kHz).

The sounds are synthesized here so the app ships with no third-party audio.
Run from the repo root:  python3 scripts/gen_sounds.py
"""
import math
import struct
import wave
from pathlib import Path

RATE = 44100
OUT = Path(__file__).resolve().parent.parent / "src-tauri" / "sounds"


def bell(freq, start, length, decay, partials=((1.0, 1.0), (2.0, 0.35), (3.01, 0.12))):
    """A soft bell note: a few partials with an exponential decay."""
    def f(t):
        t -= start
        if t < 0 or t > length:
            return 0.0
        attack = min(1.0, t / 0.006)
        env = attack * math.exp(-t / decay)
        return env * sum(a * math.sin(2 * math.pi * freq * m * t) for m, a in partials)
    return f


def render(name, notes, seconds, peak=0.55):
    n = int(RATE * seconds)
    samples = [sum(note(i / RATE) for note in notes) for i in range(n)]
    top = max(abs(s) for s in samples) or 1.0
    # Short fade-out so the file never ends on a click.
    fade = int(RATE * 0.05)
    for i in range(fade):
        samples[n - 1 - i] *= i / fade
    OUT.mkdir(parents=True, exist_ok=True)
    with wave.open(str(OUT / f"{name}.wav"), "wb") as w:
        w.setnchannels(1)
        w.setsampwidth(2)
        w.setframerate(RATE)
        w.writeframes(b"".join(struct.pack("<h", int(s / top * peak * 32767)) for s in samples))
    print("wrote", OUT / f"{name}.wav")


def tone(freq, start, length, fade=0.04):
    """A soft sine beep with smooth edges."""
    def f(t):
        t -= start
        if t < 0 or t > length:
            return 0.0
        edge = min(1.0, t / fade, (length - t) / fade)
        return edge * math.sin(2 * math.pi * freq * t)
    return f


MARIMBA = ((1.0, 1.0), (3.93, 0.25), (9.2, 0.05))
HARP = ((1.0, 1.0), (2.0, 0.5), (3.0, 0.2), (4.0, 0.08))

SOUNDS = {
    # Two rising bell notes.
    "chime": ([bell(659.25, 0.0, 1.6, 0.45), bell(987.77, 0.18, 1.6, 0.55)], 1.8),
    # One deep, long bell.
    "bell": ([bell(523.25, 0.0, 2.4, 0.9, ((1.0, 1.0), (2.76, 0.3), (5.4, 0.1), (0.5, 0.25)))], 2.5),
    # Three wooden notes going up.
    "marimba": ([bell(f, i * 0.16, 0.9, 0.18, MARIMBA) for i, f in enumerate((523.25, 659.25, 783.99))], 1.3),
    # Two pairs of gentle beeps.
    "pulse": ([tone(880, t, 0.09) for t in (0.0, 0.16, 0.6, 0.76)], 1.0),
    # A slow plucked arpeggio.
    "harp": ([bell(f, i * 0.11, 1.4, 0.35, HARP) for i, f in enumerate((392.0, 493.88, 587.33, 783.99))], 1.9),
}

if __name__ == "__main__":
    for name, (notes, seconds) in SOUNDS.items():
        render(name, notes, seconds)

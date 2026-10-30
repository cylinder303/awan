"""Write the tiny fonts the Rust tests read (src-tauri/tests/fonts/).

Each glyph is a plain box: the tests only need the names, the character
map and the weight axis, not real letter shapes. Needs fontTools:
    pip install fonttools
    python scripts/test-fonts.py
"""

from pathlib import Path

from fontTools.fontBuilder import FontBuilder
from fontTools.pens.t2CharStringPen import T2CharStringPen
from fontTools.pens.ttGlyphPen import TTGlyphPen

OUT = Path(__file__).resolve().parent.parent / "src-tauri" / "tests" / "fonts"
ARABIC = "ابتثجحخدذرزسشصضطظعغفقكلمنهوي"
DIGITS = "0123456789:"
LATIN = "Awan"


def glyph_names(text):
    return {ord(c): f"uni{ord(c):04X}" for c in text}


def box(pen, width):
    pen.moveTo((50, 0))
    pen.lineTo((50, 700))
    pen.lineTo((width - 50, 700))
    pen.lineTo((width - 50, 0))
    pen.closePath()


def names(family):
    return {"familyName": family, "styleName": "Regular", "uniqueFontIdentifier": f"{family} Test",
            "fullName": f"{family} Regular", "psName": family.replace(" ", "") + "-Regular",
            "version": "Version 1.000"}


def ttf(family, text, path):
    cmap = glyph_names(text)
    order = [".notdef", *cmap.values()]
    fb = FontBuilder(1000, isTTF=True)
    fb.setupGlyphOrder(order)
    fb.setupCharacterMap(cmap)
    glyphs = {}
    for g in order:
        pen = TTGlyphPen(None)
        box(pen, 600)
        glyphs[g] = pen.glyph()
    fb.setupGlyf(glyphs)
    fb.setupHorizontalMetrics({g: (600, 50) for g in order})
    fb.setupHorizontalHeader(ascent=800, descent=-200)
    fb.setupNameTable(names(family))
    fb.setupOS2(usWeightClass=400)
    fb.setupPost()
    fb.save(path)


def variable_otf(family, text, path):
    cmap = glyph_names(text)
    order = [".notdef", *cmap.values()]
    fb = FontBuilder(1000, isTTF=False)
    fb.setupGlyphOrder(order)
    fb.setupCharacterMap(cmap)
    charstrings = {}
    for g in order:
        pen = T2CharStringPen(None, None, CFF2=True)
        box(pen, 600)
        charstrings[g] = pen.getCharString()
    fb.setupCFF2(charstrings)
    fb.setupHorizontalMetrics({g: (600, 50) for g in order})
    fb.setupHorizontalHeader(ascent=800, descent=-200)
    fb.setupNameTable(names(family))
    fb.setupOS2(usWeightClass=400)
    fb.setupPost()
    fb.setupFvar([("wght", 200, 400, 900, "Weight")], [])
    fb.save(path)


if __name__ == "__main__":
    OUT.mkdir(parents=True, exist_ok=True)
    ttf("Awan Test Arabic", ARABIC + DIGITS + LATIN, OUT / "arabic-subset.ttf")
    ttf("Awan Test Latin", DIGITS + LATIN, OUT / "latin-subset.ttf")
    variable_otf("Awan Test Variable", DIGITS + LATIN, OUT / "variable-subset.otf")
    for f in sorted(OUT.glob("*.?tf")):
        print(f.name, f.stat().st_size, "bytes")

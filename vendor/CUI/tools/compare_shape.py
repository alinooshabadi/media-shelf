"""Compare the C font-unit shaper with uharfbuzz (test-only Python packages)."""
import argparse
import json
import itertools
import random
import subprocess
from pathlib import Path
import uharfbuzz as hb

def corpus():
    letters = "ءآأؤإئابتثجحخدذرزسشصضطظعغفقكلمنهويپچژکگیۀ"
    lines = []
    for c in letters:
        lines.extend([c, c + "ب", "ب" + c + "ب", "ب" + c])
    lines += ["سلام دنیا", "امروز ۳ تا Timer دارم", "می‌خوام", "میخوام", "لا", "Timer: سلام ۱۲۳ دنیا.", "(سلام) Timer 123",
              "لَا", "لأ لإ لآ", "بِسْمِ اللَّهِ", "نَشانهٔ فارسی", "بّ بَ بُ بِ بْ بً بٌ بٍ", "ب\u200dب", "ب\u200cب", "لا ل\u200dا ل\u200cا",
              "فارسی ۰۱۲۳۴۵۶۷۸۹ 0123456789 !؟،؛.", "ک ی ك ي", "أ إ آ ؤ ئ", "ب\u0654", "ب\u0651\u064e", "ب\u064e\u0651"]
    rng = random.Random(519)
    words = ["سلام", "دنیا", "آزمایش", "می‌خوام", "کار", "زمان", "Timer", "۱۲۳", "123", "لا", "کتاب", "یک", "دو", "گچ", "پنجره"]
    while len(lines) < 220:
        lines.append(" ".join(rng.choices(words, k=rng.randrange(2, 10))) + rng.choice(["", ".", "؟", "!"]))
    return lines

def compare(exe, font, lines):
    face = hb.Face(Path(font).read_bytes())
    hf = hb.Font(face)
    hf.scale = (face.upem, face.upem)
    result = subprocess.run([str(exe), str(font)], input="\n".join(lines) + "\n", text=True, encoding="utf-8", capture_output=True, check=True)
    outputs = result.stdout.splitlines()
    assert len(outputs) == len(lines), result
    mismatches = []
    ids = positions = 0
    for text, output in zip(lines, outputs):
        buf = hb.Buffer()
        buf.add_str(text)
        buf.direction, buf.script, buf.language = "rtl", "Arab", "und"
        hb.shape(hf, buf)
        expected = [(g.codepoint, p.x_advance, p.x_offset, p.y_offset) for g, p in zip(buf.glyph_infos, buf.glyph_positions)]
        actual = [tuple(map(int, row.split(","))) for row in output.split()[1:]]
        assert int(output.split()[0]) == len(actual)
        equal_ids = [g[0] for g in actual] == [g[0] for g in expected]
        ids += equal_ids
        positions += actual == expected
        if actual != expected:
            mismatches.append({"text": text, "glyph_ids_equal": equal_ids, "expected": expected, "actual": actual})
    return {"font": str(font), "lines": len(lines), "glyph_ids_equal": ids, "positions_equal": positions, "mismatches": mismatches}

if __name__ == "__main__":
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("executable", type=Path)
    p.add_argument("fonts", type=Path, nargs="+")
    p.add_argument("--output", type=Path, required=True)
    p.add_argument("--marks", action="store_true", help="Also compare all pairs of Arabic marks U+064B..U+065F")
    args = p.parse_args()
    lines = corpus()
    if args.marks:
        marks = [chr(cp) for cp in range(0x64b, 0x660)]
        lines += ["ب" + a + b + "ت" for a, b in itertools.product(marks, repeat=2)]
    results = [compare(args.executable, f, lines) for f in args.fonts]
    args.output.write_text(json.dumps({"harfbuzz": hb.version_string(), "results": results}, ensure_ascii=False, indent=2), encoding="utf-8")
    for r in results:
        print(r["font"], f'IDs {r["glyph_ids_equal"]}/{r["lines"]}; positions {r["positions_equal"]}/{r["lines"]}')
        for m in r["mismatches"][:3]:
            print(ascii(m["text"]), "IDs equal:", m["glyph_ids_equal"])
    raise SystemExit(any(r["positions_equal"] != r["lines"] for r in results))

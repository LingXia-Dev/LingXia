"""Pixel 3, light theme, 440 dpi: inspect verify-input.mjs screenshot evidence."""
from pathlib import Path
import re
import shutil
import xml.etree.ElementTree as ET

from PIL import Image


def caret(name):
    root = Path("target/gpu-ime-evidence")
    node = next(n for n in ET.parse(root / f"{name}.xml").iter("node")
                if n.get("resource-id", "").endswith(":id/greeting-input"))
    left, top, right, bottom = map(int, re.findall(r"\d+", node.get("bounds")))
    padding = round(12 * 440 / 160)
    phases = []
    for path in sorted(root.glob(f"{name}-[0-9].png")):
        image = Image.open(path).convert("RGB")
        runs = []
        for x in range(left + padding, right - padding + 1):
            current = 0
            for y in range(top + padding, bottom - padding):
                current = current + 1 if max(image.getpixel((x, y))) < 110 else 0
                runs.append((current, x, y + 1))
        phases.append((*max(runs), path))
    assert any(p[0] >= 44 for p in phases), f"{name}: no visible caret"
    assert any(p[0] < 44 for p in phases), f"{name}: caret did not blink off"
    longest, x, end, path = max(phases)
    shutil.copyfile(path, root / f"{name}.png")
    return x, end - longest, end, (left, top, right, bottom), padding, node.get("text")


x, _, _, (left, _, right, _), padding, _ = caret("long-input-end")
assert abs(x - (right - padding)) <= 5, (x, right, padding)
x, _, _, (left, _, right, _), padding, _ = caret("shortened-input")
assert left + padding < x < (left + right) / 2, (x, left, right)
_, start, end, (_, top, _, bottom), padding, text = caret("multiline-end")
assert text.count("\n") == 9 and text.endswith("line-09"), repr(text)
assert abs(end - (bottom - padding)) <= 3, (start, end, bottom, padding)
_, start, _, (_, top, _, _), padding, text = caret("multiline-short")
assert text == "line" and abs(start - (top + padding)) <= 3, (text, start, top, padding)
print("PASS: caret blinks in all four captures; single/multiline tails stay visible and deletion resets both offsets")

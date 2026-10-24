"""Verify GPU backdrop compositing from the physical-device UI probe captures."""
from pathlib import Path
from PIL import Image

root = Path("target")
before = Image.open(root / "gpu-independent-ui-before-popup.png").convert("RGB")
after = Image.open(root / "gpu-independent-ui-popup.png").convert("RGB")
assert before.size == after.size, "captures must share orientation and dimensions"
width, height = before.size
samples = 0
for y in range(height // 4, height * 3 // 4, max(1, height // 20)):
    for x in (width // 100, width - width // 100 - 1):
        background = before.getpixel((x, y))
        assert background == (246, 247, 251), (x, y, background)
        actual = after.getpixel((x, y))
        expected = tuple(round(channel * (1 - 128 / 255)) for channel in background)
        assert all(abs(a - e) <= 2 for a, e in zip(actual, expected)), (x, y, actual, expected)
        samples += 1
print(f"PASS {samples} physical GPU backdrop samples blend #00000080 over #f6f7fb")

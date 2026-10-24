from pathlib import Path
import re
import xml.etree.ElementTree as ET
from PIL import Image

root = Path('target/gpu-typography-evidence')
node = next(n for n in ET.parse(root / 'mono.xml').iter('node')
            if n.get('resource-id', '').endswith(':id/greeting-input'))
left, top, right, bottom = map(int, re.findall(r'\d+', node.get('bounds')))
image = Image.open(root / 'mono.png').convert('RGB')
padding, line_height = 33, 77  # 12dp and 28dp at Pixel 3's 440dpi.
starts = []
extents = []
for line in range(2):
    y0 = top + padding + line * line_height
    active = False
    columns = []
    ink_columns = []
    for x in range(left + padding, right - padding):
        ink = any(max(image.getpixel((x, y))) < 100 for y in range(y0, y0 + line_height))
        if ink and not active:
            columns.append(x)
        if ink:
            ink_columns.append(x)
        active = ink
    assert ink_columns
    extents.append(ink_columns[-1] - ink_columns[0] + 1)
    starts.append(columns[:4])
assert len(starts[0]) == 4, starts
advances = [b-a for a, b in zip(starts[0], starts[0][1:])]
assert max(advances)-min(advances) <= 1, (starts, advances)
assert all(28 <= advance <= 31 for advance in advances), advances
# Droid Sans Mono's adjacent W strokes touch; compare their full run extent.
assert abs(extents[1] - 4 * sum(advances) / len(advances)) <= 4, extents
print(f'PASS GPU mono pixels: i advances {advances}, four-W width {extents[1]} at 28dp line spacing')

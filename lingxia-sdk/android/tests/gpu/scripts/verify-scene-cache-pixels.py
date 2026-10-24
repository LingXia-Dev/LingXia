from pathlib import Path
from PIL import Image, ImageChops

before = Path('target/gpu-scene-cache-layout-before')
after = Path('target/gpu-layout-evidence')
for name, content in [('portrait', (0, 120, 1080, 2020)), ('landscape', (0, 100, 2020, 1080))]:
    old = Image.open(before / f'{name}.png').convert('RGB')
    new = Image.open(after / f'{name}.png').convert('RGB')
    assert old.size == new.size
    difference = ImageChops.difference(old.crop(content), new.crop(content))
    changed = sum(pixel != (0, 0, 0) for pixel in difference.getdata())
    assert changed == 0, (name, changed, difference.getbbox())
    print(f'PASS {name}: app content pixels match the uncached baseline exactly')

# Showcase captures

Drop one screenshot per platform here, named by its id:

```
macos.png  ios.png  android.png  windows.png  harmony.png
```

Getting started currently uses `macos.png` via `astro:assets`. Until a file
exists for a platform, do not invent a glob-driven gallery — add the image and
wire it from the page that needs it.

`.png`, `.jpg`, `.webp`, and `.avif` are all accepted.

**Capture spec** (keep it consistent and premium):

- The same showcase app and the same screen on every platform.
- Dark theme, 2× / retina, OS chrome trimmed.
- Desktop (macOS / Windows) landscape ≈ 16:10; phone (iOS / Android / Harmony) portrait ≈ 9:19.

# Showcase captures

Drop one screenshot per platform here, named by its id:

```
macos.png  ios.png  android.png  windows.png  harmony.png
```

`Showcase.astro` picks them up automatically (via `import.meta.glob`) and
optimizes them with `astro:assets` (responsive, modern formats, lazy-loaded).
Until a file exists for a platform, its panel shows a branded placeholder frame.

`.png`, `.jpg`, `.webp`, and `.avif` are all accepted.

**Capture spec** (keep it consistent and premium):

- The same showcase app and the same screen on every platform.
- Dark theme, 2× / retina, OS chrome trimmed.
- Desktop (macOS / Windows) landscape ≈ 16:10; phone (iOS / Android / Harmony) portrait ≈ 9:19.

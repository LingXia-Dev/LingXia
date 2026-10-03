# Showcase captures

Drop one screenshot per platform here, named by its id:

```
macos.png  ios.png  android.png  windows.png  harmony.png
```

None are checked in yet. Once they land, wire them into the homepage hero
(`components/Hero.astro`, replacing the illustrated app mock) and the
Getting started guide (en + zh) via `astro:assets`. Do not invent a
glob-driven gallery — add the image and wire it from the page that needs it.

`.png`, `.jpg`, `.webp`, and `.avif` are all accepted.

**Capture spec** (keep it consistent and premium):

- The same showcase app and the same screen on every platform.
- Dark theme, 2× / retina, OS chrome trimmed.
- Desktop (macOS / Windows) landscape ≈ 16:10; phone (iOS / Android / Harmony) portrait ≈ 9:19.
- No personal data on screen: no IP addresses, account names, or real
  notifications; current product tagline and version.

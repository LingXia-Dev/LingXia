This is `servo-fonts` 0.5.0 with one Android font-list fix. Current Android
`fonts.xml` files declare their fallback families, CJK included, only by
`lang` and select a `.ttc` face by `index`. Upstream skips unnamed families
and always opens face 0, so CJK text renders as missing glyphs.

The patch registers `lang`-only families as `lang:<tag>`, honors `index`, and
adds those families to the CJK fallback list. Remove it once Servo's Android
font list handles them.

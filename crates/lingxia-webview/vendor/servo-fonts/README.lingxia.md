This is `servo-fonts` 0.7.0 with temporary Android CJK and emoji fallback patches.

Android `fonts.xml` can declare fallback families with `lang` but no `name`,
and select a face within a `.ttc` collection using `index`. Servo 0.7.0 skips
families without `name` and always opens face 0. On the tested Pixel 3,
the Simplified Chinese family has `lang="zh-Hans"`, no `name`, and `index="2"`.
Skipping that family prevents its use for fallback; ignoring the index can
select glyph forms for the wrong language.

The patch registers language-only families as `lang:<tag>`, honors `index`,
and adds those families to the CJK fallback list. Emoji presentation selects
`lang:und-Zsye`, with `Noto Color Emoji` as a named-family alternative.
Repeated family names are merged in XML order: Android can put ordinary emoji
and flags in separate `und-Zsye` families, and both must reach glyph matching.
Text presentation (including U+FE0E) does not select the emoji fallback.
The `und-Zsym` family supplies monochrome symbols, including text-presentation
hearts and suns. Hangul syllables and Jamo explicitly select `lang:ko` because
Servo's `is_cjk` helper excludes most Hangul blocks.
This does not replace Android's system font matching or add glyphs missing
from the device's installed fonts.

DOM text keeps supported ZWJ emoji sequences in one glyph cluster. Servo 0.7.0's
separate Canvas 2D run builder can still split ZWJ sequences; this font-list
patch does not change `servo-script`'s canvas segmentation.

## Upstream tracking

- [Issue #33322](https://github.com/servo/servo/issues/33322): Android cannot
  render non-Latin characters. The discussion identifies unnamed fallback
  families being skipped by the XML parser.
- [Issue #44725](https://github.com/servo/servo/issues/44725): tracks Android
  font-list generation and documents the same parsing problem.
- [PR #42287](https://github.com/servo/servo/pull/42287): proposes using
  `ASystemFontIterator` and `AFontMatcher` for system font enumeration and
  fallback matching, and declares that it fixes #33322.

As checked on 2026-10-08, #33322 is open, #44725 is closed, and #42287 is
still open and unmerged. The closed tracking issue does not mean the fix
is available in Servo 0.7.0.

Remove this vendor override once the Servo version we use includes an
equivalent fix. Before removal, verify Simplified Chinese, Traditional
Chinese, Japanese, and Korean fallback on Android, including the selected
TTC face and language-specific glyph forms. Also verify color emoji, skin-tone
modifiers, ZWJ sequences, flags, and text/emoji presentation selectors.

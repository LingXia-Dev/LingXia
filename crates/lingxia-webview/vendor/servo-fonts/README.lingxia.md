This is `servo-fonts` 0.7.0 with a temporary Android CJK font fallback patch.

Android `fonts.xml` can declare fallback families with `lang` but no `name`,
and select a face within a `.ttc` collection using `index`. Servo 0.7.0 skips
families without `name` and always opens face 0. On the tested Pixel 3,
the Simplified Chinese family has `lang="zh-Hans"`, no `name`, and `index="2"`.
Skipping that family prevents its use for fallback; ignoring the index can
select glyph forms for the wrong language.

The patch registers language-only families as `lang:<tag>`, honors `index`,
and adds those families to the CJK fallback list. It does not provide a
complete replacement for Android's system font matching.

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
TTC face and language-specific glyph forms.

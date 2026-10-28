Corpus pages port the constructs a production app uses (the h-app review inventory) into
GPU Views: shells, sheets, chat, rich text, forms, long lists, charts and RTL. They are test
inputs for `corpus/run.mjs`, not examples; data comes from `corpus/states/corpus/`.

`node corpus/run.mjs --visual` compares every render with its fingerprint in `corpus/visual/` (16-px
block means as a small PNG, with a few levels of tolerance for anti-aliasing) and outlines changed blocks
in `target/corpus/visual-diff/`. After an intended visual change, re-record with `--update-visual` and
look at the diffs before committing.

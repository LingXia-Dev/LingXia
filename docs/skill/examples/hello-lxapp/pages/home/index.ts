// Logic layer. Runs in the native JS runtime (not the WebView).
// `Page` is a global from @lingxia/types — no import needed.
// `_`-prefixed methods are private to Logic and NOT exposed as View actions.

interface HomeData {
  count: number;
  message: string;
}

Page<HomeData>({
  data: {
    count: 0,
    message: "Tap to increment",
  },

  onLoad(options) {
    // `options` is the URL query that opened this page.
    // Useful for routing — ignored in the simplest case.
  },

  increment() {
    // `this.data` is read-only. Always go through `setData` so the View
    // sees the change.
    this.setData({ count: this.data.count + 1 });
  },

  reset() {
    this.setData({ count: 0 });
  },

  _doubled() {
    // Private helper. Not visible to View.
    return this.data.count * 2;
  },
});

// Demonstrates what survives a navigation and what does not: leaving this page
// ends its instance, so `data` and the rendered document both come back fresh.
// The instance tag is mirrored through storage, which is app-scoped and does
// survive, so the page can show you the tag it had last time.
const LAST_INSTANCE_TAG_KEY = "lifecycle:lastInstanceTag";
const MAX_EVENTS = 8;

function newInstanceTag() {
  return Math.random().toString(16).slice(2, 6).toUpperCase();
}

Page({
  data: {
    instanceTag: "",
    previousInstanceTag: "",
    logicCounter: 0,
    events: [] as string[],
  },

  onLoad: function () {
    const tag = newInstanceTag();
    this.setData({ instanceTag: tag });
    this._record("onLoad");

    const storage = lx.getStorage();
    storage
      .get(LAST_INSTANCE_TAG_KEY)
      .then((previous) => {
        this.setData({ previousInstanceTag: (previous as string) || "—" });
        return storage.set(LAST_INSTANCE_TAG_KEY, tag);
      })
      .catch((err) => console.warn("[Lifecycle] instance tag storage failed", err));
  },

  onReady: function () {
    this._record("onReady");
  },

  onShow: function () {
    this._record("onShow");
  },

  onHide: function () {
    this._record("onHide");
  },

  onUnload: function () {
    this._record("onUnload");
  },

  bumpLogicCounter: function () {
    this.setData({ logicCounter: this.data.logicCounter + 1 });
  },

  goBack: function () {
    lx.navigateBack();
  },

  // The log lives in `data`, so it only ever shows the current instance's
  // events — hide/show repeat within one instance, unload ends it.
  _record: function (event: string) {
    const stamp = new Date().toLocaleTimeString();
    this.setData({
      events: [...this.data.events, `${stamp}  ${event}`].slice(-MAX_EVENTS),
    });
  },
});

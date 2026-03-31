Page({
  data: {
    maxLengthValue: "",
    syncValue: "",
    controlledValue: "",
    autoBlurValue: "",
    autoBlurFocus: false,
  },

  onLoad: function () {
    try {
      lx.setNavigationBarTitle({ title: "Input" });
    } catch (error) {
      console.warn("setNavigationBarTitle failed:", error);
    }
  },

  onInputChange: function (detail) {
    if (detail?.value === undefined) return;
  },

  onMaxLengthInput: function (detail) {
    if (detail?.value === undefined) return;
    var nextValue = String(detail.value);
    if (nextValue === this.data.maxLengthValue) return;
    this.setData({ maxLengthValue: nextValue });
  },

  onSyncInput: function (detail) {
    if (detail?.value === undefined) return;
    var nextValue = String(detail.value);
    if (nextValue === this.data.syncValue) return;
    this.setData({ syncValue: nextValue });
  },

  onControlledInput: function (detail) {
    if (detail?.value === undefined) return;
    var value = String(detail.value).replace(/11/g, "2");
    if (value === this.data.controlledValue) return;
    this.setData({ controlledValue: value });
  },

  onAutoBlurInput: function (detail) {
    if (detail?.value === undefined) return;
    var value = String(detail.value).replace(/\D/g, "").slice(0, 3);
    if (value === "123") {
      if (this.data.autoBlurValue === value && this.data.autoBlurFocus === false) return;
      this.setData({ autoBlurValue: value, autoBlurFocus: false });
      return;
    }
    if (this.data.autoBlurValue === value) return;
    this.setData({ autoBlurValue: value });
  },

  onAutoBlurFocus: function (_detail) {},

  onAutoBlurBlur: function (_detail) {},
});

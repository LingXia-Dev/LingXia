Page({
  data: {
    demoType: "",
    maxLengthValue: "",
    textareaMaxLengthValue: "",
    syncValue: "",
    controlledValue: "",
    autoBlurValue: "",
    autoBlurFocus: false,
  },

  onLoad: function (options) {
    var mode = options && options.type === "textarea" ? "textarea" : "input";
    this.setData({
      demoType: mode,
      maxLengthValue: "",
      textareaMaxLengthValue: "",
      syncValue: "",
      controlledValue: "",
      autoBlurValue: "",
      autoBlurFocus: false,
    });
    this.applyNavigationTitle(mode);
  },

  onShow: function () {
    if (this.data.demoType) {
      this.applyNavigationTitle(this.data.demoType);
    }
  },

  applyNavigationTitle: function (mode) {
    var title = mode === "textarea" ? "Textarea" : "Input";
    try {
      lx.setNavigationBarTitle({ title: title });
    } catch (error) {
      console.warn("setNavigationBarTitle failed:", error);
    }
  },

  onInputChange: function (event) {
    var detail = event?.detail || {};
    if (detail.value === undefined) return;
  },

  onMaxLengthInput: function (event) {
    var detail = event?.detail || {};
    if (detail.value === undefined) return;
    var nextValue = String(detail.value);
    if (nextValue === this.data.maxLengthValue) return;
    this.setData({ maxLengthValue: nextValue });
  },

  onSyncInput: function (event) {
    var detail = event?.detail || {};
    if (detail.value === undefined) return;
    var nextValue = String(detail.value);
    if (nextValue === this.data.syncValue) return;
    this.setData({ syncValue: nextValue });
  },

  onControlledInput: function (event) {
    var detail = event?.detail || {};
    if (detail.value === undefined) return;
    var value = String(detail.value).replace(/11/g, "2");
    if (value === this.data.controlledValue) return;
    this.setData({ controlledValue: value });
  },

  onAutoBlurInput: function (event) {
    var detail = event?.detail || {};
    if (detail.value === undefined) return;
    var value = String(detail.value).replace(/\D/g, "").slice(0, 3);
    if (value === "123") {
      if (this.data.autoBlurValue === value && this.data.autoBlurFocus === false) return;
      this.setData({ autoBlurValue: value, autoBlurFocus: false });
      return;
    }
    if (this.data.autoBlurValue === value) return;
    this.setData({ autoBlurValue: value });
  },

  onAutoBlurFocus: function (_event) {},

  onAutoBlurBlur: function (_event) {},

  onTextareaMaxLengthInput: function (event) {
    var detail = event?.detail || {};
    if (detail.value === undefined) return;
    var nextValue = String(detail.value);
    if (nextValue === this.data.textareaMaxLengthValue) return;
    this.setData({ textareaMaxLengthValue: nextValue });
  },

});

const tabs = ["selector", "multiSelector", "time", "date"];

Page({
  data: {
    activeTab: "selector",
    coffee: undefined,
    location: undefined,
    multiTime: ["09", "30"],
    time: undefined,
    year: undefined,
    month: undefined,
    date: undefined,
    dateRange: undefined,
  },

  onLoad: function () {},

  setActiveTab: function (params = {}) {
    const next = params?.tab;
    if (typeof next !== "string" || !tabs.includes(next)) {
      return;
    }
    this.setData({ activeTab: next });
  },

  _applyPickerValue: function ({
    field,
    value,
  }: {
    field: string;
    value: any;
  }) {
    switch (field) {
      case "coffee":
        this.setData({ coffee: value });
        return;
      case "location":
        this.setData({ location: value });
        return;
      case "multiTime":
        this.setData({ multiTime: value });
        return;
      case "time":
        this.setData({ time: value });
        return;
      case "year":
        this.setData({ year: value });
        return;
      case "month":
        this.setData({ month: value });
        return;
      case "date":
        this.setData({ date: value });
        return;
      case "dateRange":
        this.setData({ dateRange: value });
        return;
      default:
        return;
    }
  },

  onPickerConfirm: function (params: { field?: string; value?: any } = {}) {
    var field = params?.field;
    var value = params?.value;
    if (!field || value === undefined) return;
    this._applyPickerValue({ field, value });
  },

  onPickerScroll: function (params: { field?: string; value?: any } = {}) {
    var field = params?.field;
    var value = params?.value;
    if (!field || value === undefined) return;
    this._applyPickerValue({ field, value });
  },
});

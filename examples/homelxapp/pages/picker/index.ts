const coffees = ["Espresso", "Americano", "Latte", "Cappuccino", "Mocha", "Macchiato"];
const continents = ["Asia", "Europe", "America", "Africa"];
const cities = {
  Asia: ["Beijing", "Tokyo", "Seoul", "Singapore"],
  Europe: ["London", "Paris", "Berlin", "Rome"],
  America: ["New York", "Los Angeles", "Toronto", "Mexico City"],
  Africa: ["Cairo", "Lagos", "Nairobi", "Johannesburg"],
};
const hours = Array.from({ length: 24 }, (_, i) => i.toString().padStart(2, "0"));
const minutes = Array.from({ length: 60 }, (_, i) => i.toString().padStart(2, "0"));
const tabs = ["selector", "multiSelector", "time", "date"];

function normalizeField(raw) {
  if (typeof raw !== "string") return "";
  return raw.trim();
}

function asIndexList(index) {
  if (Array.isArray(index)) {
    return index.map((value) => Number(value) || 0);
  }
  if (typeof index === "number") {
    return [index];
  }
  return [];
}

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

  resolvePickerValue: function (field, detail = {}) {
    if (detail.value !== undefined) {
      return detail.value;
    }

    const indexList = asIndexList(detail.index);
    if (field === "coffee") {
      const value = coffees[indexList[0] || 0];
      return value ?? undefined;
    }

    if (field === "location") {
      const continent = continents[indexList[0] || 0];
      const cityList = cities[continent] || [];
      const city = cityList[indexList[1] || 0];
      if (!continent || !city) return undefined;
      return [continent, city];
    }

    if (field === "multiTime") {
      const h = hours[indexList[0] || 0];
      const m = minutes[indexList[1] || 0];
      if (!h || !m) return undefined;
      return [h, m];
    }

    return undefined;
  },

  applyPickerValue: function (field, value) {
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

  handlePickerEvent: function (event = {}) {
    const detail = event?.detail || {};
    if (detail.cancelled === true) {
      return;
    }

    const dataset = event?.currentTarget?.dataset || event?.target?.dataset || {};
    const field = normalizeField(dataset.field);
    if (!field) {
      return;
    }

    const value = this.resolvePickerValue(field, detail);
    if (value === undefined) {
      return;
    }
    this.applyPickerValue(field, value);
  },

  onPickerChange: function (event = {}) {
    this.handlePickerEvent(event);
  },

  onPickerScroll: function (event = {}) {
    this.handlePickerEvent(event);
  },
});

import { definePage } from '@lingxia/types/page';
const tabs = ["selector", "multiSelector", "time", "date"];

export default definePage({
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
  setup(page) {
    const helpers = {
      applyPickerValue({
        field,
        value,
      }: {
        field: string;
        value: any;
      }) {
        switch (field) {
          case "coffee":
            page.update(d => { d.coffee = value; });
            return;
          case "location":
            page.update(d => { d.location = value; });
            return;
          case "multiTime":
            page.update(d => { d.multiTime = value; });
            return;
          case "time":
            page.update(d => { d.time = value; });
            return;
          case "year":
            page.update(d => { d.year = value; });
            return;
          case "month":
            page.update(d => { d.month = value; });
            return;
          case "date":
            page.update(d => { d.date = value; });
            return;
          case "dateRange":
            page.update(d => { d.dateRange = value; });
            return;
          default:
            return;
        }
      },
    };
    const actions = {
      setActiveTab(params: { tab?: string } = {}) {
        const next = params?.tab;
        if (typeof next !== "string" || !tabs.includes(next)) {
          return;
        }
        page.update(d => { d.activeTab = next; });
      },
      onPickerConfirm(params: { field?: string; value?: any } = {}) {
        var field = params?.field;
        var value = params?.value;
        if (!field || value === undefined) return;
        helpers.applyPickerValue({ field, value });
      },
      onPickerScroll(params: { field?: string; value?: any } = {}) {
        var field = params?.field;
        var value = params?.value;
        if (!field || value === undefined) return;
        helpers.applyPickerValue({ field, value });
      },
    };
    return {
      onLoad() {},
      actions,
    };
  },
});

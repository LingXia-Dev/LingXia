import { definePage } from '@lingxia/logic';
import type { PickerField, PickerPage, PickerTab } from './contract';

const TABS: readonly PickerTab[] = ['selector', 'multiSelector', 'time', 'date'];
const LIST_FIELDS: readonly PickerField[] = ['location', 'multiTime', 'dateRange'];

const initialData: PickerPage['data'] = { activeTab: 'selector', multiTime: ['09', '30'] };

export default definePage({
  data: initialData,
  setup: page => ({
    actions: {
      setActiveTab({ tab }: { tab: PickerTab }) {
        if (TABS.includes(tab)) page.update(d => { d.activeTab = tab; });
      },
      // Confirm and each column scroll both land here: the page shows the latest value.
      setValue({ field, value }: { field: PickerField; value: string | string[] }) {
        if (LIST_FIELDS.includes(field) !== Array.isArray(value)) return;
        page.update(d => { Object.assign(d, { [field]: value }); });
      },
    },
  }),
});

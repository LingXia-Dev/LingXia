import type { PageContract } from '@lingxia/types/page';

export type PickerTab = 'selector' | 'multiSelector' | 'time' | 'date';

/** One stored value per demo picker. */
export interface PickerValues {
  coffee?: string;
  location?: string[];
  multiTime: string[];
  time?: string;
  year?: string;
  month?: string;
  date?: string;
  dateRange?: string[];
}

export type PickerField = keyof PickerValues;

export type PickerPage = PageContract<
  PickerValues & { activeTab: PickerTab },
  {
    setActiveTab(params: { tab: PickerTab }): Promise<void>;
    setValue(params: { field: PickerField; value: string | string[] }): Promise<void>;
  }
>;

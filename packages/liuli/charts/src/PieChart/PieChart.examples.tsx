import { useLocal } from '@lingxia/view';
import { Held } from '@liulidesign/core/internal/example';
import { summarize } from '../summary/en';
import { PieChart } from './PieChart';

const spending = [
  { category: 'Rent', amount: 1450 }, { category: 'Food', amount: 620 }, { category: 'Transport', amount: 210 },
  { category: 'Leisure', amount: 180 }, { category: 'Savings', amount: 400 },
];
const grouped = (value: number) => String(value).replace(/\B(?=(\d{3})+(?!\d))/g, ',');
const dollars = (value: number) => `$${grouped(value)}`;
const percent = (share: number) => `${Math.round(share * 100)}%`;

/** A row in the legend chooses its part; the hole then shows that part in place of the total. */
export function Spending() {
  const part = useLocal<string | null>('part', null);
  return (
    <PieChart
      rows={spending}
      nameField="category"
      valueField="amount"
      label="Spending by category"
      title="Spending"
      description="October"
      formatValue={dollars}
      formatShare={percent}
      totalLabel="Total"
      summarize={summarize}
      value={part.value}
      onChange={part.set}
      data-testid="spending"
    />
  );
}

const storage = [{ kind: 'Photos', gb: 38 }, { kind: 'Apps', gb: 21 }, { kind: 'Messages', gb: 9 }, { kind: 'Free', gb: 60 }];
const gigabytes = (value: number) => `${value} GB`;

export function Solid() {
  return <PieChart rows={storage} nameField="kind" valueField="gb" label="Storage by kind" title="Storage" variant="pie" formatValue={gigabytes} formatShare={percent} summarize={summarize} data-testid="storage" />;
}

const languages = [
  { language: 'English', readers: 4210 }, { language: 'Spanish', readers: 1870 }, { language: 'Chinese', readers: 1540 }, { language: 'Hindi', readers: 960 },
  { language: 'French', readers: 610 }, { language: 'German', readers: 480 }, { language: 'Japanese', readers: 350 }, { language: 'Korean', readers: 190 },
];

/** Eight parts are too many to tell apart: the four largest keep a slice, the rest share one. */
export function Merged() {
  return (
    <PieChart
      rows={languages}
      nameField="language"
      valueField="readers"
      label="Readers by language"
      title="Readers by language"
      formatValue={grouped}
      formatShare={percent}
      otherLabel="Other"
      maxParts={4}
      totalLabel="Readers"
      summarize={summarize}
      data-testid="languages"
    />
  );
}

export function Sizes() {
  return (
    <view className="gap-6">
      <PieChart rows={storage} nameField="kind" valueField="gb" label="Storage by kind, small" size="sm" formatValue={gigabytes} totalLabel="Used" legendPosition="none" />
      <PieChart rows={storage} nameField="kind" valueField="gb" label="Storage by kind, large" size="lg" formatValue={gigabytes} totalLabel="Used" legendPosition="none" />
    </view>
  );
}

export function StateSelected() {
  return <PieChart rows={storage} nameField="kind" valueField="gb" label="Storage by kind" size="sm" formatValue={gigabytes} totalLabel="Used" value="Apps" legendPosition="none" data-testid="selected" />;
}

export function StateWhole() {
  return <PieChart rows={storage.slice(0, 1)} nameField="kind" valueField="gb" label="Storage by kind" size="sm" formatValue={gigabytes} totalLabel="Used" legendPosition="none" data-testid="whole" />;
}

export function StateLoading() {
  return <PieChart rows={[]} nameField="kind" valueField="gb" label="Storage by kind" title="Storage" description="By kind of file" size="sm" isLoading data-testid="loading" />;
}

export function StateEmpty() {
  return <PieChart rows={[]} nameField="kind" valueField="gb" label="Storage by kind" title="Storage" description="By kind of file" size="sm" emptyLabel="Nothing stored yet" emptyHint="Files you upload are counted here." data-testid="empty" />;
}

export function StateError() {
  return <Held value={true}>{(hasFailed, set) => <PieChart rows={storage} nameField="kind" valueField="gb" label="Storage by kind" title="Storage" description="By kind of file" size="sm" error={hasFailed ? "Storage could not be read." : undefined} onRetry={() => set(false)} retryLabel="Try again" data-testid="error" />}</Held>;
}

/** The home tile. */
export function Specimen() {
  return <PieChart rows={storage} nameField="kind" valueField="gb" label="Storage" size="xs" legendPosition="none" className="w-28 self-center" />;
}

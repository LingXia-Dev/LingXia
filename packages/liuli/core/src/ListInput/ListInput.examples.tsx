import { useLocal } from '@lingxia/view';
import { unused } from '../example';
import { Select } from '../Select/Select';
import { TextInput } from '../TextInput/TextInput';
import { ListInput, type ListInputProps } from './ListInput';

interface Phone { id: string; kind: string; number: string }
const kinds = [{ value: 'mobile', label: 'Mobile' }, { value: 'home', label: 'Home' }, { value: 'work', label: 'Work' }];

/** Phone numbers: each row is a kind and a number; one row always stays, and four is the most. */
export function Phones() {
  const phones = useLocal<readonly Phone[]>('phones', [
    { id: 'p1', kind: 'mobile', number: '912 345 678' },
    { id: 'p2', kind: 'work', number: '' },
  ]);
  const next = useLocal('next', 3);
  const change = (index: number, patch: Partial<Phone>) => phones.set(phones.value.map((phone, at) => (at === index ? { ...phone, ...patch } : phone)));
  return (
    <ListInput
      label="Phone numbers"
      description="We call only about a delivery."
      rowKeys={phones.value.map(phone => phone.id)}
      renderRow={index => (
        // At large text the kind keeps its words whole and the number wraps under it.
        <view className="flex-row flex-wrap gap-2">
          <Select label={`Kind of phone ${index + 1}`} isLabelHidden items={kinds} value={phones.value[index].kind} onChange={kind => change(index, { kind })} className="min-w-28" />
          <TextInput label={`Phone ${index + 1}`} isLabelHidden type="tel" placeholder="Number" value={phones.value[index].number} onChange={number => change(index, { number })} className="grow basis-32" data-testid={`phone-${index + 1}`} />
        </view>
      )}
      onAdd={() => { phones.set([...phones.value, { id: `p${next.value}`, kind: 'mobile', number: '' }]); next.set(next.value + 1); }}
      addLabel="Add a phone number"
      onRemove={index => phones.set(phones.value.filter((_, at) => at !== index))}
      removeLabel={index => `Remove phone ${index + 1}`}
      min={1}
      max={4}
      data-testid="phones"
    />
  );
}

/** Nothing yet, and required: the error says what to add. */
export function Links() {
  const links = useLocal<readonly { id: string; url: string }[]>('links', []);
  const next = useLocal('next', 1);
  return (
    <ListInput
      label="Links"
      isRequired
      status={links.value.length === 0 ? { type: 'error', message: 'Add at least one link.' } : undefined}
      rowKeys={links.value.map(link => link.id)}
      renderRow={index => (
        <TextInput label={`Link ${index + 1}`} isLabelHidden type="text" autoCapitalize="none" placeholder="https://" value={links.value[index].url} onChange={url => links.set(links.value.map((link, at) => (at === index ? { ...link, url } : link)))} />
      )}
      onAdd={() => { links.set([...links.value, { id: `l${next.value}`, url: '' }]); next.set(next.value + 1); }}
      addLabel="Add a link"
      onRemove={index => links.set(links.value.filter((_, at) => at !== index))}
      removeLabel={index => `Remove link ${index + 1}`}
      data-testid="links"
    />
  );
}

export function StateDisabled() {
  return (
    <ListInput
      label="Phone numbers"
      isDisabled
      disabledMessage="Ask an admin to change these."
      rowKeys={['p1']}
      renderRow={() => <TextInput label="Phone 1" isLabelHidden isDisabled value="+351 912 345 678" onChange={unused} />}
      onAdd={unused}
      addLabel="Add a phone number"
      onRemove={unused}
      removeLabel={index => `Remove phone ${index + 1}`}
    />
  );
}

/** The home tile. */
export function Specimen() {
  return (
    <view className="w-full gap-1.5">
      <view className="flex-row items-center gap-1.5">
        <view className="h-6 grow basis-0 rounded-control border border-line-strong bg-surface" />
        <view className="size-3 rounded-full bg-text/20" />
      </view>
      <view className="flex-row items-center gap-1.5">
        <view className="h-6 grow basis-0 rounded-control border border-line-strong bg-surface" />
        <view className="size-3 rounded-full bg-text/20" />
      </view>
      <view className="h-5 w-14 rounded-control border border-line bg-surface" />
    </view>
  );
}

/** The playground's props that are not plain values. */
export function playground() {
  return {
    rowKeys: ['p1', 'p2'],
    renderRow: (index: number) => <TextInput label={`Phone ${index + 1}`} isLabelHidden type="tel" placeholder="Number" value="" onChange={unused} />,
    onAdd: unused,
    onRemove: unused,
    removeLabel: (index: number) => `Remove phone ${index + 1}`,
  } satisfies Partial<ListInputProps>;
}

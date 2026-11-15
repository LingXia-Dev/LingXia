import { Badge } from '../Badge/Badge';
import { MetadataList } from './MetadataList';

/** What is known about a record: the labels in one column, the values in the next. */
export function Invoice() {
  return (
    <MetadataList
      label="Invoice details"
      items={[
        { key: 'customer', label: 'Customer', value: 'Blue Harbor Ltd' },
        { key: 'status', label: 'Status', value: 'Overdue', content: <Badge label="Overdue" tone="danger" size="sm" /> },
        { key: 'issued', label: 'Issued', value: '9 October 2026' },
        { key: 'due', label: 'Due', value: '9 November 2026' },
        { key: 'address', label: 'Billing address', value: '14 Quay Street, Harbour Buildings, Unit 3, Bristol BS1 4DA' },
      ]}
      data-testid="invoice"
    />
  );
}

/** Each label over its value, two to a line: for a few short facts at the top of a page. */
export function Stacked() {
  return (
    <MetadataList
      label="File details"
      layout="stacked"
      items={[
        { key: 'size', label: 'Size', value: '4.2 MB' },
        { key: 'kind', label: 'Kind', value: 'PDF document' },
        { key: 'created', label: 'Created', value: '12 March 2026' },
        { key: 'owner', label: 'Owner', value: 'Ana Lima' },
      ]}
      data-testid="file"
    />
  );
}

/** The home tile. */
export function Specimen() {
  return <MetadataList label="Details" className="w-32" items={[{ key: 'a', label: 'Size', value: '4.2 MB' }, { key: 'b', label: 'Kind', value: 'PDF' }]} />;
}

import { useLocal } from '@lingxia/view';
import { Breadcrumb } from './Breadcrumb';

/** Where a file sits: each level above it is a way back. */
export function Path() {
  const opened = useLocal('opened', '');
  const go = (name: string) => () => opened.set(name);
  return (
    <view className="gap-2">
      <Breadcrumb
        label="Breadcrumb"
        items={[
          { key: 'home', label: 'Home', onClick: go('Home') },
          { key: 'projects', label: 'Projects', onClick: go('Projects') },
          { key: 'plan', label: 'Q3 plan' },
        ]}
        data-testid="path"
      />
      {opened.value !== '' && <text role="status" className="text-footnote text-muted">{`Opened ${opened.value}`}</text>}
    </view>
  );
}

/** A deep trail: the middle levels fold into a menu, and the first level and the last two stay. */
export function Deep() {
  const none = () => {};
  return (
    <Breadcrumb
      label="Breadcrumb"
      moreLabel="More levels"
      items={[
        { key: 'drive', label: 'Drive', onClick: none },
        { key: 'clients', label: 'Clients', onClick: none },
        { key: 'northwind', label: 'Northwind', onClick: none },
        { key: '2026', label: '2026', onClick: none },
        { key: 'invoices', label: 'Invoices', onClick: none },
        { key: 'inv', label: 'INV-1043.pdf' },
      ]}
      data-testid="deep"
    />
  );
}

/** The home tile. */
export function Specimen() {
  return <Breadcrumb label="Breadcrumb" items={[{ key: 'a', label: 'Home', onClick: () => {} }, { key: 'b', label: 'Plan' }]} />;
}

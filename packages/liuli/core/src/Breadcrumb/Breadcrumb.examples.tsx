import { useLocal } from '@lingxia/view';
import { Said, unused } from '../example';
import { Breadcrumb, type BreadcrumbProps } from './Breadcrumb';

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

/** Levels that are pages of the app: each is a link with `to`. */
export function Pages() {
  return (
    <Breadcrumb
      label="Breadcrumb"
      items={[
        { key: 'home', label: 'Home', to: { page: 'index' } },
        { key: 'settings', label: 'Settings', to: { page: 'settings' } },
        { key: 'privacy', label: 'Privacy' },
      ]}
      data-testid="pages"
    />
  );
}

/** A deep trail: the middle levels fold into a menu, and the first level and the last two stay. */
export function Deep() {
  return (
    <Said>
      {say => (
        <Breadcrumb
          label="Breadcrumb"
          moreLabel="More levels"
          items={[
            { key: 'drive', label: 'Drive', onClick: () => say('Opened Drive') },
            { key: 'clients', label: 'Clients', onClick: () => say('Opened Clients') },
            { key: 'northwind', label: 'Northwind', onClick: () => say('Opened Northwind') },
            { key: '2026', label: '2026', onClick: () => say('Opened 2026') },
            { key: 'invoices', label: 'Invoices', onClick: () => say('Opened Invoices') },
            { key: 'inv', label: 'INV-1043.pdf' },
          ]}
          data-testid="deep"
        />
      )}
    </Said>
  );
}

/** The home tile. */
export function Specimen() {
  return <Breadcrumb label="Breadcrumb" items={[{ key: 'a', label: 'Home', onClick: unused }, { key: 'b', label: 'Plan' }]} />;
}

/** The playground's props that are not plain values. */
export function playground() {
  return {
    items: [
      { key: 'home', label: 'Home', onClick: unused },
      { key: 'projects', label: 'Projects', onClick: unused },
      { key: 'plan', label: 'Q3 plan' },
    ],
  } satisfies Partial<BreadcrumbProps>;
}

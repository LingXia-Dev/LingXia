// A footer row as the todo page draws it: count, a three-segment filter and a ghost action. No
// segment may be narrower than its label; a row too narrow overflows instead of clipping text.
const filters = ['All', 'Active', 'Completed'];

function Segmented({ selected }: { selected: string }) {
  return (
    <row className="gap-1 p-1 bg-surface-sunken rounded-xl">
      {filters.map(label => (
        <button key={label} onClick={() => {}}
          className={label === selected
            ? 'grow basis-0 min-h-8 px-2 py-1 rounded-lg text-xs font-medium text-center bg-surface shadow-sm text-text'
            : 'grow basis-0 min-h-8 px-2 py-1 rounded-lg text-xs font-medium text-center bg-transparent text-muted'}>
          {label}
        </button>
      ))}
    </row>
  );
}

function Footer({ width, count }: { width: string; count: string }) {
  return (
    <row className={`${width} flex-row items-center justify-between gap-3 px-5 py-4 border border-line rounded-lg`}>
      <text className="text-sm text-muted">{count}</text>
      <Segmented selected="All" />
      <button onClick={() => {}} className="px-3 py-2 rounded-lg text-sm font-medium text-accent">Clear completed</button>
    </row>
  );
}

export function render() {
  return (
    <page className="bg-surface">
      <scroll className="grow">
        <column className="p-4 gap-4 items-start">
          <Footer width="w-[361px]" count="3 items left" />
          <Footer width="w-[300px]" count="12 items left" />
          <row className="w-[260px] p-2 border border-line rounded-lg">
            <Segmented selected="Completed" />
          </row>
          {/* Opting in: a text that may shrink truncates beside a segmented control that may not. */}
          <row className="w-[300px] items-center gap-2 p-2 border border-line rounded-lg">
            <text className="min-w-0 shrink truncate text-sm">A long project name that cannot fit</text>
            <Segmented selected="Active" />
          </row>
        </column>
      </scroll>
    </page>
  );
}

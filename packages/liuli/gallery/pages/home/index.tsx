import type { View } from '@lingxia/view';

type Actions = { open(target: { page: string }): Promise<void> };

const components = [
  { page: 'button', name: 'Button', summary: 'Runs an action, with variants, sizes and a loading state' },
  { page: 'field', name: 'Field', summary: 'Label, help text and status around a control' },
  { page: 'text-input', name: 'TextInput', summary: 'One-line text editor, bound or controlled' },
  { page: 'checkbox-input', name: 'CheckboxInput', summary: 'One on/off choice with help text' },
  { page: 'dialog', name: 'Dialog', summary: 'Modal panel for a decision or a short task' },
  { page: 'dropdown-menu', name: 'DropdownMenu', summary: 'A button that opens a menu of actions' },
] as const;

export function render({ actions }: View<Actions>) {
  return (
    <page className="bg-canvas">
      <scroll className="grow">
        <view className="p-4 gap-2" role="list">
          <text role="heading" headingLevel={1} className="text-2xl font-semibold pb-2">Liuli</text>
          {components.map(item => (
            <button
              key={item.page}
              role="listitem"
              data-testid={`open-${item.page}`}
              onClick={() => actions.open({ page: item.page })}
              className="items-start gap-0.5 rounded-xl border border-line bg-surface px-4 py-3 active:bg-surface-sunken"
            >
              <text className="text-base font-medium">{item.name}</text>
              <text className="text-sm text-muted">{item.summary}</text>
            </button>
          ))}
        </view>
      </scroll>
    </page>
  );
}

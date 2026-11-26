import { Said } from '../example';
import { Button } from '../Button/Button';
import { DropdownMenu } from '../DropdownMenu/DropdownMenu';
import { IconButton } from '../IconButton/IconButton';
import { Token } from '../Token/Token';
import { OverflowList, type OverflowListProps } from './OverflowList';

const tags = ['Design', 'Research', 'Mobile', 'Accessibility', 'Performance', 'Charts', 'Localisation', 'Testing'];

/** Tags on one line: those that do not fit become a count. */
export function Tags() {
  return (
    <OverflowList
      label="Tags"
      items={tags}
      keyOf={tag => tag}
      renderItem={tag => <Token label={tag} size="sm" />}
      renderOverflow={hidden => <Token label={`+${hidden.length}`} size="sm" data-testid="tags-more" />}
      data-testid="tags"
    />
  );
}

const actions = [
  { id: 'reply', label: 'Reply', icon: 'reply' },
  { id: 'forward', label: 'Forward', icon: 'forward' },
  { id: 'archive', label: 'Archive', icon: 'archive' },
  { id: 'pin', label: 'Pin', icon: 'pin' },
  { id: 'delete', label: 'Delete', icon: 'delete' },
] as const;

/** A toolbar narrower than its actions: the last ones move into a More menu, in the same order. */
export function Actions() {
  return (
    <Said>
      {say => (
        <OverflowList
          label="Message actions"
          items={actions}
          keyOf={action => action.id}
          renderItem={action => <Button label={action.label} startIcon={action.icon} size="sm" variant="secondary" onClick={() => say(action.label)} data-testid={`action-${action.id}`} />}
          renderOverflow={hidden => (
            <DropdownMenu label="More actions" size="sm" items={hidden.map(action => ({ id: action.id, label: action.label, icon: action.icon, onClick: () => say(action.label) }))}>
              <IconButton label="More actions" icon="more" size="sm" data-testid="actions-more" />
            </DropdownMenu>
          )}
          data-testid="actions"
        />
      )}
    </Said>
  );
}

const path = ['Home', 'Projects', 'LingXia', 'Design', 'Components', 'OverflowList'];

/** Folding from the start keeps the end of a path, where you are. */
export function FromStart() {
  return (
    <OverflowList
      items={path}
      keyOf={step => step}
      collapseFrom="start"
      minVisible={1}
      renderItem={(step, index) => (
        <view className="flex-row items-center gap-2">
          {index > 0 && <text aria-hidden className="text-footnote text-faint">/</text>}
          <text className={index === path.length - 1 ? 'text-footnote font-medium text-text' : 'text-footnote text-muted'}>{step}</text>
        </view>
      )}
      renderOverflow={hidden => <text aria-label={`${hidden.length} more`} className="text-footnote text-muted">…</text>}
      data-testid="path"
    />
  );
}

/** The home tile. */
export function Specimen() {
  return <view className="w-56"><Tags /></view>;
}

export function playground() {
  return {
    items: tags,
    renderItem: (tag: string) => <Token label={tag} size="sm" />,
    renderOverflow: (hidden: readonly string[]) => <Token label={`+${hidden.length}`} size="sm" />,
  } satisfies Partial<OverflowListProps<string>>;
}


import { useLocal, useView, type View } from '@lingxia/view';
import { ListDetail } from '@liulidesign/core/ListDetail';
import { Split } from '@liulidesign/core/Split';
import { Button } from '@liulidesign/core/Button';
import { EmptyState } from '@liulidesign/core/EmptyState';
import { Item, ItemList } from '@liulidesign/core/Item';
import '../../theme/index.css';

interface OrderItem {
  readonly id: string;
  readonly title: string;
  readonly status: string;
  readonly detail: string;
}

export function render({ data, actions }: View) {
  const { env } = useView();
  const mode = useLocal<'list-detail' | 'split'>('adaptive-mode', 'list-detail');
  const notesOpen = useLocal('adaptive-notes-open', false);
  const items = (data.items as readonly OrderItem[] | undefined) ?? [];
  const selectedId = (data.selectedId as string | null | undefined) ?? null;
  const current = items.find(item => item.id === selectedId) ?? null;
  const isCompact = env.sizeClass === 'compact';

  return (
    <page
      title="Adaptive Layout"
      headerActions={
        <Button
          size="sm"
          variant="secondary"
          label={mode.value === 'list-detail' ? 'Split' : 'ListDetail'}
          onClick={() => mode.set(mode.value === 'list-detail' ? 'split' : 'list-detail')}
          data-testid="adaptive-mode-toggle"
        />
      }
      data-testid="adaptive-page"
    >
      <view className="p-3 border-b border-line bg-surface-sunken grid grid-cols-1 regular:grid-cols-2 gap-2">
        <text data-testid="adaptive-size-class" className="font-mono text-xs text-muted">
          {`sizeClass=${env.sizeClass ?? 'compact'} · viewport=${Math.round(env.viewport?.width ?? 0)}×${Math.round(env.viewport?.height ?? 0)}`}
        </text>
        <text data-testid="adaptive-hinge" className="font-mono text-xs text-muted">
          {env.hinge ? `hinge=${env.hinge.orientation}/${env.hinge.posture}` : 'hinge=none'}
        </text>
      </view>

      {mode.value === 'list-detail' ? (
        <ListDetail
          value={selectedId}
          onChange={id => actions.selectItem({ id })}
          listLabel="Orders"
          data-testid="adaptive-list-detail"
        >
          <ListDetail.List>
            <scroll className="grow">
              <ItemList>
                {items.map(item => (
                  <Item
                    key={item.id}
                    label={item.title}
                    description={item.status}
                    isSelected={selectedId === item.id}
                    onClick={() => actions.selectItem({ id: item.id })}
                    data-testid={`adaptive-item-${item.id}`}
                  />
                ))}
              </ItemList>
            </scroll>
          </ListDetail.List>
          <ListDetail.Detail
            title={current?.title}
            placeholder={<EmptyState title="Select an order" description="Single-file adaptive page powered by <ListDetail>." />}
          >
            {current && (
              <view className="p-4 gap-2" data-testid="adaptive-detail-body">
                <text className="text-base font-semibold text-text">{current.title}</text>
                <text className="text-xs font-mono text-muted">{current.status}</text>
                <text className="text-sm text-text">{current.detail}</text>
              </view>
            )}
          </ListDetail.Detail>
        </ListDetail>
      ) : (
        <Split
          compact="sheet"
          bindOpen={notesOpen}
          asideTitle="Live Notes"
          asideWidth={320}
          aside={
            <view className="p-4 gap-2">
              <text className="text-sm font-semibold text-text">Companion Aside</text>
              <text className="text-xs text-muted">
                Side-by-side on regular, upper/lower leaves on half-opened tabletop folds, and BottomSheet on compact.
              </text>
            </view>
          }
          data-testid="adaptive-split"
        >
          <view className="grow p-4 gap-3">
            <text className="text-base font-semibold text-text">Primary Canvas</text>
            {isCompact && (
              <Button
                variant="secondary"
                label="Open Notes"
                width="content"
                onClick={() => notesOpen.set(true)}
                data-testid="adaptive-open-notes"
              />
            )}
          </view>
        </Split>
      )}
    </page>
  );
}

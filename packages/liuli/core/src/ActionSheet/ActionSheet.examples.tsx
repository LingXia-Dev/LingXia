import { useLocal } from '@lingxia/view';
import { Button } from '../Button/Button';
import { actionSheetRecipe } from './ActionSheet.recipe';
import { ActionSheet, type ActionSheetProps } from './ActionSheet';
import { unused } from '../example';
import { Glyph } from '../internal/Glyph';

/** What to do with a photo: a few actions with icons, the destructive one last, Cancel apart. */
export function Basic() {
  const open = useLocal('open', false);
  const done = useLocal('done', 'Nothing yet');
  const act = (what: string) => () => done.set(what);
  return (
    <view className="gap-3">
      <Button label="Photo options" width="content" onClick={() => open.set(true)} data-testid="photo-open" />
      <text role="status" className="text-footnote text-muted" data-testid="photo-done">{done.value}</text>
      <ActionSheet isOpen={open.value} onOpenChange={next => open.set(next)} label="This photo"
        items={[
          { label: 'Share', icon: 'share', onClick: act('Shared'), 'data-testid': 'photo-share' },
          { label: 'Duplicate', icon: 'copy', onClick: act('Duplicated'), 'data-testid': 'photo-duplicate' },
          { label: 'Add to album', icon: 'folder', onClick: act('Added to an album'), 'data-testid': 'photo-album' },
          { label: 'Delete', icon: 'delete', variant: 'destructive', onClick: act('Deleted'), 'data-testid': 'photo-delete' },
        ]}
        data-testid="photo"
      />
    </view>
  );
}

/** A question with its consequence spelled out under the title; the rows are its answers. */
export function WithMessage() {
  const open = useLocal('open', false);
  const done = useLocal('done', 'Draft open');
  return (
    <view className="gap-3">
      <Button label="Close draft" width="content" onClick={() => open.set(true)} data-testid="draft-open" />
      <text role="status" className="text-footnote text-muted" data-testid="draft-done">{done.value}</text>
      <ActionSheet isOpen={open.value} onOpenChange={next => open.set(next)} label="Close this draft?"
        description="Unsaved changes are lost if you discard it."
        items={[
          { label: 'Save draft', onClick: () => done.set('Draft saved'), 'data-testid': 'draft-save' },
          { label: 'Discard', variant: 'destructive', onClick: () => done.set('Draft discarded'), 'data-testid': 'draft-discard' },
        ]}
        data-testid="draft"
      />
    </view>
  );
}

/** No title to draw: the sheet keeps it as its name, and the rows speak for themselves. */
export function Untitled() {
  const open = useLocal('open', false);
  const sort = useLocal('sort', 'Newest first');
  return (
    <view className="gap-3">
      <Button label={sort.value} endIcon="chevron-down" width="content" onClick={() => open.set(true)} data-testid="order-open" />
      <ActionSheet isOpen={open.value} onOpenChange={next => open.set(next)} label="Order" isLabelHidden
        items={['Newest first', 'Oldest first', 'By name'].map(name => ({ label: name, onClick: () => sort.set(name), 'data-testid': `order-${name}` }))}
        data-testid="order"
      />
    </view>
  );
}

/** The parts, drawn still on a phone's bottom edge. */
export function Parts() {
  const r = actionSheetRecipe;
  return (
    <view className="justify-end rounded-container bg-scrim px-4 pt-12 overflow-hidden">
      <view className="w-full gap-2 border border-b-0 border-line bg-surface-overlay shadow-modal px-3 pt-2 pb-3 rounded-t-dialog">
        <view className={r.grabber.base} />
        <view className={r.header.base}>
          <text className={r.title.base}>This photo</text>
        </view>
        <view className={r.actions.base}>
          <view className="flex-row items-center gap-3 self-stretch px-3 min-h-control-lg text-text">
            <Glyph name="share" className={r.itemIcon.base} />
            <text className={r.itemLabel.base}>Share</text>
          </view>
          <view className="flex-row items-center gap-3 self-stretch px-3 min-h-control-lg text-danger">
            <Glyph name="delete" className={r.itemIcon.base} />
            <text className={r.itemLabel.base}>Delete</text>
          </view>
        </view>
        <view className={r.cancel.base}>
          <view className="flex-row items-center gap-3 self-stretch px-3 min-h-control-lg text-text">
            <view className={r.itemIcon.base} />
            <text className={r.itemLabel.base}>Cancel</text>
          </view>
        </view>
      </view>
    </view>
  );
}

/** The home tile: a still drawing, since a real sheet covers the page. */
export function Specimen() {
  return (
    <view className="w-30 h-20 justify-end overflow-hidden rounded-control bg-text/6">
      <view className="px-2.5 pt-1.5 pb-2 gap-1.5 rounded-t-control border border-b-0 border-line bg-surface-overlay shadow-[0_-2px_8px_rgb(0_0_0/0.10)]">
        <view className="self-center w-5 h-0.5 rounded-full bg-text/20" />
        <view className="h-1.5 w-3/5 rounded-full bg-text/10" />
        <view className="h-1.5 w-1/2 rounded-full bg-danger/40" />
        <view className="h-1.5 w-1/3 rounded-full bg-text/10" />
      </view>
    </view>
  );
}

/** The playground's props that are not plain values. */
export function playground() {
  return {
    items: [
      { label: 'Share', icon: 'share', onClick: unused },
      { label: 'Delete', icon: 'delete', variant: 'destructive', onClick: unused },
    ],
  } satisfies Partial<ActionSheetProps>;
}

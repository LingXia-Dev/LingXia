import { unused } from '../example';
import type { BottomSheetProps } from './BottomSheet';
import { useLocal } from '@lingxia/view';
import { Button } from '../Button/Button';
import { LayerContext } from '../internal/layer';
import { Item, ItemList } from '../Item/Item';
import { bottomSheetRecipe } from './BottomSheet.recipe';
import { BottomSheet } from './BottomSheet';

const moveTargets = ['Inbox', 'Travel', 'Work', 'Recipes'];

/** Choosing where a note goes: a sheet as tall as its list, dragged down or tapped outside to cancel. */
export function MoveTo() {
  const open = useLocal('open', false);
  const folder = useLocal('folder', 'Inbox');
  return (
    <view className="gap-3">
      <Button label="Move note" width="content" onClick={() => open.set(true)} data-testid="move-open" />
      <text className="text-footnote text-muted" data-testid="folder">{`In ${folder.value}`}</text>
      <BottomSheet isOpen={open.value} onOpenChange={next => open.set(next)} label="Move to" data-testid="move">
        <ItemList label="Folders">
          {moveTargets.map(name => (
            <Item key={name} label={name} onClick={() => { folder.set(name); open.set(false); }} data-testid={`folder-${name}`} />
          ))}
        </ItemList>
      </BottomSheet>
    </view>
  );
}

const comments = [
  ['Ana', 'Can we move the review to Thursday?'],
  ['Kai', 'Thursday works. I will update the plan.'],
  ['Mira', 'Added the budget numbers under Q3.'],
  ['Ana', 'Thanks. The hiring section still needs dates.'],
  ['Kai', 'I will add them after the sync.'],
  ['Mira', 'Linked the research notes as well.'],
] as const;

/** A long list: the sheet opens halfway and drags up to nearly the full height. */
export function Comments() {
  const open = useLocal('open', false);
  return (
    <view className="gap-3">
      <Button label="Show comments" width="content" onClick={() => open.set(true)} data-testid="comments-open" />
      <BottomSheet
        isOpen={open.value}
        onOpenChange={next => open.set(next)}
        label="Comments"
        closeLabel="Close"
        detents="expandable"
        data-testid="comments"
      >
        {comments.map(([who, text], index) => (
          <view key={`c${index}`} className="gap-0.5">
            <text className="text-footnote font-semibold text-text">{who}</text>
            <text className="text-body text-text">{text}</text>
          </view>
        ))}
      </BottomSheet>
    </view>
  );
}

/** A step that must be answered: no drag off, no tap outside. */
export function Required() {
  const open = useLocal('open', false);
  const answer = useLocal('answer', '');
  return (
    <view className="gap-3">
      <Button label="Leave notebook" variant="destructive" width="content" onClick={() => open.set(true)} data-testid="leave-open" />
      {answer.value !== '' && <text className="text-footnote text-muted" data-testid="answer">{answer.value}</text>}
      <BottomSheet
        isOpen={open.value}
        onOpenChange={next => open.set(next)}
        label="Leave this notebook?"
        description="You lose access to its notes until someone invites you again."
        dismissOn="none"
        data-testid="leave"
        actions={
          <>
            <Button label="Stay" onClick={() => { answer.set('Stayed'); open.set(false); }} data-testid="stay" />
            <Button label="Leave" variant="destructive" onClick={() => { answer.set('Left'); open.set(false); }} data-testid="leave-confirm" />
          </>
        }
      />
    </view>
  );
}

/** The sheet drawn at rest on a scrim, for the gallery's parts stage: its own part classes, in the page. */
export function Parts() {
  return (
    <view className="justify-end rounded-container bg-scrim px-4 pt-12 overflow-hidden">
      <view className="w-full px-5 pt-2 pb-5 gap-4 rounded-t-dialog border border-b-0 border-text/12 bg-surface-overlay shadow-modal">
        <view className={bottomSheetRecipe.grabber.base} />
        <view className={bottomSheetRecipe.header.base}>
          <text className={bottomSheetRecipe.title.base}>Move to</text>
        </view>
        <LayerContext.Provider value="overlay">
          <ItemList label="Folders">
            <Item label="Inbox" />
            <Item label="Travel" />
          </ItemList>
        </LayerContext.Provider>
      </view>
    </view>
  );
}

/** The home tile: a still drawing, since a real sheet covers the page. */
export function Specimen() {
  return (
    <view className="w-30 h-20 justify-end overflow-hidden rounded-control bg-text/6">
      <view className="px-2.5 pt-1.5 pb-2.5 gap-1.5 rounded-t-control border border-b-0 border-text/12 bg-surface-overlay shadow-[0_-2px_8px_rgb(0_0_0/0.10)]">
        <view className="self-center w-5 h-0.5 rounded-full bg-text/20" />
        <text className="text-caption font-semibold text-text">Move to</text>
        <view className="h-1.5 w-3/4 rounded-full bg-text/10" />
        <view className="h-1.5 w-1/2 rounded-full bg-text/10" />
      </view>
    </view>
  );
}

/** The playground's props that are not plain values. */
export function playground() {
  return {
    children: (
      <ItemList label="Folders">
        {moveTargets.map(name => <Item key={name} label={name} onClick={unused} />)}
      </ItemList>
    ),
  } satisfies Partial<BottomSheetProps>;
}

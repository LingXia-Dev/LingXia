// The View catalogue's one demo shape, the group page built from it and the index of groups.
import { useView, type Child, type Component } from '@lingxia/view';
import { Page } from '../ui';
import { chrome, demos, groups, missing, say, type GroupId, type Words } from './text';


/** A plain title, one sentence on what to try, then the live demo on a card. */
export function Demo({ title, sentence, testId, children }: { title: string; sentence: string; testId: string; children?: Child }) {
  return (
    <view className="gap-2">
      <view className="gap-1 px-1">
        <text role="heading" headingLevel={2} className="text-base font-semibold">{title}</text>
        <text className="text-sm text-muted">{sentence}</text>
      </view>
      <view data-testid={testId} className="p-4 gap-3 rounded-2xl border border-line bg-surface">{children}</view>
    </view>
  );
}

/** One group of the catalogue: its demos in order, then what the group does not show yet. */
export function GroupPage<G extends GroupId>({ id, bodies }: { id: G; bodies: Record<keyof (typeof demos)[G], Component> }) {
  const t = say(useView().env.displayLanguage);
  const words = demos[id] as Record<string, { testId: string; title: Words; try: Words }>;
  const drawn = bodies as Record<string, Component>;
  return (
    <Page testId={`view-group-${id}`}>
      {Object.keys(words).map(key => {
        const Body = drawn[key];
        return <Demo key={key} title={t(words[key].title)} sentence={t(words[key].try)} testId={words[key].testId}><Body /></Demo>;
      })}
      {missing[id].length > 0 ? (
        <view data-testid="view-group-missing" className="gap-1 px-1 pt-2">
          <text className="text-xs font-semibold text-muted">{t(chrome.missing)}</text>
          {missing[id].map(item => <text key={item.en} className="text-xs text-faint">{t(item)}</text>)}
        </view>
      ) : null}
    </Page>
  );
}

/** The catalogue's first screen: one row per group, with what it holds and how many demos. */
export function Index({ onOpen }: { onOpen: (page: string) => void }) {
  const t = say(useView().env.displayLanguage);
  return (
    <Page testId="view-index" intro={t(chrome.index)}>
      <view role="list" className="rounded-2xl border border-line bg-surface divide-y divide-line">
        {groups.map(group => (
          <button key={group.id} role="listitem" data-testid={`view-index-${group.id}`} onClick={() => onOpen(group.page)}
            className="flex-row items-center gap-3 px-4 py-3 active:bg-surface-sunken">
            <view className="grow basis-0 min-w-0 gap-0.5 items-start">
              <text className="text-base font-medium text-start">{t(group.title)}</text>
              <text className="text-sm text-muted text-start">{t(group.line)}</text>
            </view>
            <text data-testid={`view-index-${group.id}-count`} className="text-sm text-muted tabular-nums">{Object.keys(demos[group.id]).length}</text>
          </button>
        ))}
      </view>
    </Page>
  );
}

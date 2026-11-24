import { t, useAction, useLocal, type MessageKey } from '@lingxia/view';
import type { View } from './$types';

export function render({ data, actions }: View) {
  const query = useLocal('query', '');
  const openUrl = useAction(actions.openUrl);
  const reopenTab = useAction(actions.reopenTab);
  const openPage = useAction(actions.openPage);

  const submit = () => {
    const val = query.value.trim();
    if (!val) return;
    void openUrl.run({ url: val });
    query.set('');
  };

  return (
    <page data-testid="newtab-page" className="bg-canvas text-text items-center px-6 py-10 gap-8">
      <view className="w-full max-w-3xl gap-6">
        <view className="items-center gap-2">
          <text className="text-4xl font-semibold tracking-tight">{data.appName}</text>
          {data.version ? <text className="text-xs text-muted">{data.version}</text> : null}
        </view>

        <view className="flex-row items-center gap-2 rounded-xl bg-surface border border-line px-4 py-2">
          <input
            data-testid="newtab-url-input"
            aria-label={t('newtab.placeholder')}
            value={query.value}
            placeholder={t('newtab.placeholder')}
            onInput={(v) => query.set(v)}
            onSubmit={submit}
            className="grow basis-0 min-w-0 text-base bg-transparent"
          />
          <button
            data-testid="newtab-go-btn"
            onClick={submit}
            className="px-4 py-2 rounded-lg bg-accent text-on-accent text-sm font-medium"
          >
            <text>{t('newtab.go')}</text>
          </button>
        </view>

        <view className="flex-row flex-wrap gap-2 justify-center">
          {(['bookmarks', 'history', 'downloads', 'settings'] as const).map((name) => (
            <button
              key={name}
              data-testid={`newtab-nav-${name}`}
              onClick={() => { void openPage.run({ page: name }); }}
              className="px-4 py-2 rounded-lg bg-surface border border-line text-sm"
            >
              <text>{t(`nav.${name}` as MessageKey)}</text>
            </button>
          ))}
        </view>

        {data.shortcuts.length > 0 ? (
          <view className="gap-3">
            <text className="text-sm font-semibold text-muted">{t('newtab.shortcuts')}</text>
            <view className="flex-row flex-wrap gap-3">
              {data.shortcuts.map((item) => (
                <button
                  key={item.id}
                  onClick={() => { void openUrl.run({ url: item.url }); }}
                  className="px-4 py-3 rounded-xl bg-surface border border-line gap-1 max-w-xs"
                >
                  <text className="text-sm font-medium">{item.title || item.url}</text>
                  <text className="text-xs text-muted">{item.url}</text>
                </button>
              ))}
            </view>
          </view>
        ) : null}

        {data.recentlyClosed.length > 0 ? (
          <view className="gap-2">
            <text className="text-sm font-semibold text-muted">{t('newtab.recentlyClosed')}</text>
            <view className="rounded-xl bg-surface border border-line">
              {data.recentlyClosed.map((tab) => (
                <button
                  key={tab.id}
                  onClick={() => { void reopenTab.run({ id: tab.id }); }}
                  className="flex-row items-center justify-between px-4 py-2.5 border-t border-line bg-transparent"
                >
                  <text className="text-sm">{tab.title || tab.url}</text>
                  <text className="text-xs text-muted">{tab.url}</text>
                </button>
              ))}
            </view>
          </view>
        ) : null}
      </view>
    </page>
  );
}

import { useAction, useLocal } from '@lingxia/view';
import '../../theme/index.css';
import { Button, Card, Notice, Segmented } from '../../shared/ui';
import { resolveDisplayLanguage } from '../../shared/display-language';
import { getMessages } from './messages';
import type { View } from './$types';

export function render({ data, actions, env }: View) {
  const name = useLocal('home-name', '');
  const send = useAction(actions.greet);
  const photo = useLocal('home-photo', false);
  const language = resolveDisplayLanguage(data.displayLanguage.resolved ?? env.displayLanguage);
  const { t } = getMessages(language);
  const trimmed = name.value.trim();
  const greet = () => {
    if (trimmed && !send.pending) send.run({ name: trimmed });
  };
  const greeting = send.error ? t('sendFailed') : data.greeting;
  return (
    <page data-testid="home-page" className="bg-canvas">
      {Boolean(data.imageUrl) && <image data-testid="home-background" src={data.imageUrl} alt="" className="absolute inset-0 object-cover" onLoad={() => photo.set(true)} onError={() => photo.set(false)} />}
      {photo.value && <view data-testid="home-background-overlay" className="absolute inset-0 bg-linear-to-b from-transparent via-transparent to-scrim" />}
      <scroll data-testid="home-scroll" className="grow basis-0">
        <view className="min-h-full px-5 py-10 items-center justify-center gap-4">
          <Card testId="home-card" className="w-full max-w-sm p-6 gap-3 bg-surface/80 backdrop-blur-xl shadow-lg border-line/60">
            <view data-testid="home-header" className="items-center gap-1 pb-3">
              <image data-testid="home-logo" src="public/AppIcon.png" alt="" className="w-16 h-16 mb-2 rounded-2xl object-contain" />
              <text data-testid="home-title" className="text-[17px] font-semibold">
                LingXia
              </text>
              <text data-testid="home-tagline" className="text-[13px] text-muted text-center line-clamp-2">
                {t('tagline')}
              </text>
            </view>
            <input
              data-testid="home-name"
              data-controlled-value={name.value}
              aria-label={t('nameLabel')}
              bind={name}
              disabled={send.pending}
              placeholder={t('namePlaceholder')}
              onSubmit={greet}
              className="min-h-11 px-4 py-2 rounded-lg bg-surface-sunken text-[17px] placeholder:text-muted focus:ring-2 focus:ring-focus/30"
            />
            <Button testId="home-greet" onClick={greet} disabled={!trimmed || send.pending} className="min-h-12 text-[17px] font-semibold">
              {send.pending ? t('sending') : t('sayHello')}
            </Button>
            {Boolean(greeting) && (
              <Notice tone={send.error ? 'error' : 'success'} testId="home-greeting">
                {greeting}
              </Notice>
            )}
            <view data-testid="home-appearance" className="gap-1.5 pt-2">
              <view className="flex-row justify-between">
                <text className="text-[11px] font-medium text-muted">{t('appearance')}</text>
                <text data-testid="home-appearance-resolved" className="text-[11px] text-muted">
                  {env.theme === 'dark' ? t('appearanceDark') : t('appearanceLight')}
                </text>
              </view>
              <Segmented
                testId="home-appearance"
                value={data.appearance.preference}
                onChange={preference => actions.setAppearance({ preference })}
                options={[
                  { value: 'auto', label: t('appearanceAuto') },
                  { value: 'light', label: t('appearanceLight') },
                  { value: 'dark', label: t('appearanceDark') },
                ]}
              />
            </view>
            <view data-testid="home-language" className="gap-1.5 pt-1">
              <view className="flex-row justify-between">
                <text className="text-[11px] font-medium text-muted">{t('language')}</text>
                <text data-testid="home-language-resolved" className="text-[11px] text-muted">
                  {language === 'zh-CN' ? t('languageZh') : t('languageEn')}
                </text>
              </view>
              <Segmented
                testId="home-language"
                value={data.displayLanguage.preference}
                onChange={preference => actions.setDisplayLanguage({ preference })}
                options={[
                  { value: 'auto', label: t('languageAuto') },
                  { value: 'en-US', label: t('languageEn') },
                  { value: 'zh-CN', label: t('languageZh') },
                ]}
              />
            </view>
            {Boolean(data.appVersion) && (
              <text data-testid="home-version" className="text-[10px] font-medium text-muted">
                {data.appVersion}
              </text>
            )}
          </Card>
          {Boolean(data.ipAddr) && (
            <view className="flex-row max-w-full px-4 py-2 gap-2 items-center rounded-full bg-surface/70 backdrop-blur-md">
              <view className="shrink-0 w-1.5 h-1.5 rounded-full bg-success" />
              <text className="shrink-0 text-xs font-medium tracking-wide text-text">{t('myIp')}</text>
              <text data-testid="home-ip" className="min-w-0 grow truncate text-xs font-mono text-muted">
                {data.ipAddr}
              </text>
            </view>
          )}
        </view>
      </scroll>
    </page>
  );
}

import { useLocal, type View } from '@lingxia/ui';
import '../../theme/index.css';
import { Button, Card, Notice, Segmented } from '../../shared/ui';
import { resolveDisplayLanguage } from '../../shared/display-language';
import type { HomeActions, HomePageData } from '../../shared/home-types';
import { getMessages } from './messages';

export function render({ data, actions, env }: View<HomeActions, HomePageData>) {
  const name = useLocal('home-name', '');
  const sending = useLocal('home-sending', false);
  const failed = useLocal('home-error', false);
  const photo = useLocal('home-photo', false);
  const language = resolveDisplayLanguage(data.displayLanguage.resolved ?? env.displayLanguage);
  const { t } = getMessages(language);
  const trimmed = name.value.trim();
  const greet = async () => {
    if (!trimmed || sending.value) return;
    sending.set(true);
    failed.set(false);
    try {
      await actions.greet({ name: trimmed });
    } catch {
      failed.set(true);
    } finally {
      sending.set(false);
    }
  };
  const greeting = failed.value ? t('sendFailed') : data.greeting;
  return (
    <page testId="home-page" className="bg-canvas">
      {Boolean(data.imageUrl) && <image testId="home-background" src={data.imageUrl} alt="" className="absolute inset-0 object-cover" onLoad={() => photo.set(true)} onError={() => photo.set(false)} />}
      {photo.value && <column testId="home-background-overlay" className="absolute inset-0 bg-linear-to-b from-transparent via-transparent to-scrim" />}
      <scroll testId="home-scroll" className="grow basis-0">
        <column className="min-h-full px-5 py-10 items-center justify-center gap-4">
          <Card testId="home-card" className="w-full max-w-sm p-6 gap-3 bg-surface/80 backdrop-blur-xl shadow-lg border-line/60">
            <column testId="home-header" className="items-center gap-1 pb-3">
              <image testId="home-logo" src="public/AppIcon.png" alt="" className="w-16 h-16 mb-2 rounded-2xl object-contain" />
              <text testId="home-title" className="text-[17px] font-semibold">
                LingXia
              </text>
              <text testId="home-tagline" className="text-[13px] text-muted text-center line-clamp-2">
                {t('tagline')}
              </text>
            </column>
            <input
              testId="home-name"
              accessibilityLabel={t('nameLabel')}
              bind={name}
              disabled={sending.value}
              placeholder={t('namePlaceholder')}
              onSubmit={greet}
              className="min-h-11 px-4 py-2 rounded-lg bg-surface-sunken text-[17px] placeholder:text-muted focus:ring-2 focus:ring-focus/30"
            />
            <Button testId="home-greet" onClick={greet} disabled={!trimmed || sending.value} className="min-h-12 text-[17px] font-semibold">
              {sending.value ? t('sending') : t('sayHello')}
            </Button>
            {Boolean(greeting) && (
              <Notice tone={failed.value ? 'error' : 'success'} testId="home-greeting">
                {greeting}
              </Notice>
            )}
            <column testId="home-appearance" className="gap-1.5 pt-2">
              <row className="justify-between">
                <text className="text-[11px] font-medium text-muted">{t('appearance')}</text>
                <text testId="home-appearance-resolved" className="text-[11px] text-muted">
                  {data.appearance.resolved === 'dark' ? t('appearanceDark') : t('appearanceLight')}
                </text>
              </row>
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
            </column>
            <column testId="home-language" className="gap-1.5 pt-1">
              <row className="justify-between">
                <text className="text-[11px] font-medium text-muted">{t('language')}</text>
                <text testId="home-language-resolved" className="text-[11px] text-muted">
                  {language === 'zh-CN' ? t('languageZh') : t('languageEn')}
                </text>
              </row>
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
            </column>
            {Boolean(data.appVersion) && (
              <text testId="home-version" className="text-[10px] font-medium text-muted">
                {data.appVersion}
              </text>
            )}
          </Card>
          {Boolean(data.ipAddr) && (
            <row className="max-w-full px-4 py-2 gap-2 items-center rounded-full bg-surface/70 backdrop-blur-md">
              <spacer className="shrink-0 w-1.5 h-1.5 rounded-full bg-success" />
              <text className="shrink-0 text-xs font-medium tracking-wide text-text">{t('myIp')}</text>
              <text testId="home-ip" className="min-w-0 grow truncate text-xs font-mono text-muted">
                {data.ipAddr}
              </text>
            </row>
          )}
        </column>
      </scroll>
    </page>
  );
}

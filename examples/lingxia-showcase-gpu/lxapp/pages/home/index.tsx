import { PageContent, contentBounds } from '../../shared/components/page';
import { useLocal, type View } from '@lingxia/ui';
import '../../theme/index.css';
import { Icon } from '../../shared/components/icon';
import { resolveDisplayLanguage } from '../../shared/display-language';
import type { HomeActions, HomePageData } from '../../shared/home-types';
import { getMessages } from './messages';

export function render({ data, actions, env }: View<HomeActions, HomePageData>) {
  const name = useLocal('home-name', '');
  const sending = useLocal('home-sending', false);
  const error = useLocal('home-error', false);
  const appearance = data.appearance;
  const language = data.displayLanguage;
  const { t } = getMessages(resolveDisplayLanguage(language.resolved ?? env.displayLanguage));
  const greet = async () => {
    if (!name.value.trim() || sending.value) return;
    sending.set(true); error.set(false);
    try { await actions.greet({ name: name.value.trim() }); }
    catch { error.set(true); }
    finally { sending.set(false); }
  };
  const { width, height } = contentBounds(env);
  const greeting = error.value ? t('sendFailed') : data.greeting;
  return <stack testId="home-page" className="bg-background">
    {Boolean(data.imageUrl) && <image testId="home-background" src={data.imageUrl} alt="" fit="cover" style={{ width: env.viewport.width, height: env.viewport.height }} />}
    <column testId="home-background-overlay" className="bg-linear-to-b from-black/10 via-transparent to-black/40" style={{ width: env.viewport.width, height: env.viewport.height }} />
    <PageContent>
        <scroll testId="home-scroll" style={{ width, height }}>
          <column className="px-5 py-10 items-center justify-center gap-4" style={{ minHeight: height }}>
            <column testId="home-card" className="bg-surface/80 backdrop-blur-xl shadow-lg rounded-2xl border border-white/20 p-6" style={{ width: Math.min(360, Math.max(1, width - 40)) }}>
              <column testId="home-header" className="items-center text-center pb-6">
                <column className="pb-3"><image testId="home-logo" src="public/AppIcon.png" alt="" fit="contain" className="w-16 h-16 rounded-2xl" /></column>
                <text testId="home-title" className="text-[17px] font-semibold text-gray-900">LingXia</text>
                <text testId="home-tagline" lines={2} className="pt-0.5 text-[13px] text-gray-500">{t('tagline')}</text>
              </column>
              <column className="gap-3">
              <input accessibilityLabel={t('nameLabel')} testId="home-name" bind={name} enabled={!sending.value} placeholder={t('namePlaceholder')} onSubmit={greet} className="min-h-11 px-4 py-2 bg-surface-100/80 rounded-[10px] text-[17px] text-gray-900 placeholder:text-gray-400 focus:ring-2 focus:ring-blue-500/30" />
              <button testId="home-greet" onClick={greet} enabled={!!name.value.trim() && !sending.value} className={name.value.trim() && !sending.value ? 'min-h-[50px] px-4 py-3 bg-primary text-white rounded-xl text-[17px] font-semibold' : 'min-h-[50px] px-4 py-3 bg-primary/50 text-white rounded-xl text-[17px] font-semibold'}>{sending.value ? t('sending') : t('sayHello')}</button>
              </column>
              {Boolean(greeting) && <column className="pt-4"><column className={error.value ? "p-4 bg-red-50 border border-red-200 rounded-xl" : "p-4 bg-green-50 border border-green-200 rounded-xl"}>
                <row className="items-start gap-3">
                  <column className="pt-0.5 shrink-0"><Icon name={error.value ? "alert" : "success"} className={error.value ? "w-5 h-5 text-red-500 dark:text-red-400" : "w-5 h-5 text-green-500 dark:text-green-400"} /></column>
                  <text testId="home-greeting" className={error.value ? "grow basis-0 break-words text-sm text-red-700 dark:text-red-400 leading-relaxed" : "grow basis-0 break-words text-sm text-green-700 dark:text-green-400 leading-relaxed"} style={{ minWidth: 0 }}>{greeting}</text>
                </row>
              </column></column>}
              <column testId="home-appearance" className="gap-1.5 pt-4">
                <row className="justify-between">
                  <text className="text-[11px] font-medium text-gray-500">{t('appearance')}</text>
                  <text testId="home-appearance-resolved" className="text-[11px] text-gray-400">{appearance.resolved === 'dark' ? t('appearanceDark') : t('appearanceLight')}</text>
                </row>
                <row className="gap-1 p-1 bg-surface-100 rounded-[10px]">
                  {(['auto', 'light', 'dark'] as const).map(value => <button key={value} testId={`home-appearance-${value}`} onClick={() => actions.setAppearance({ preference: value })} className={appearance.preference === value ? 'grow basis-0 min-h-8 px-0 py-1 rounded-lg text-[13px] font-medium bg-surface shadow-sm text-gray-900' : 'grow basis-0 min-h-8 px-0 py-1 rounded-lg text-[13px] font-medium bg-transparent text-gray-500'}>{t(value === 'auto' ? 'appearanceAuto' : value === 'light' ? 'appearanceLight' : 'appearanceDark')}</button>)}
                </row>
              </column>
              <column testId="home-language" className="gap-1.5 pt-3">
                <row className="justify-between">
                  <text className="text-[11px] font-medium text-gray-500">{t('language')}</text>
                  <text testId="home-language-resolved" className="text-[11px] text-gray-400">{resolveDisplayLanguage(language.resolved ?? env.displayLanguage) === 'zh-CN' ? t('languageZh') : t('languageEn')}</text>
                </row>
                <row className="gap-1 p-1 bg-surface-100 rounded-[10px]">
                  {(['auto', 'en-US', 'zh-CN'] as const).map(value => <button key={value} testId={`home-language-${value}`} onClick={() => actions.setDisplayLanguage({ preference: value })} className={language.preference === value ? 'grow basis-0 min-h-8 px-0 py-1 rounded-lg text-[13px] font-medium bg-surface shadow-sm text-gray-900' : 'grow basis-0 min-h-8 px-0 py-1 rounded-lg text-[13px] font-medium bg-transparent text-gray-500'}>{t(value === 'auto' ? 'languageAuto' : value === 'en-US' ? 'languageEn' : 'languageZh')}</button>)}
                </row>
              </column>
              {Boolean(data.appVersion) && <text testId="home-version" className="pt-1 text-[10px] leading-none font-medium text-gray-500">{data.appVersion}</text>}
            </column>
            {Boolean(data.ipAddr) && <row className="px-4 py-2 bg-black/20 backdrop-blur-md rounded-[9999px] gap-2 items-center">
              <spacer className="w-1.5 h-1.5 bg-green-400 rounded-full" />
              <text className="text-xs font-medium tracking-wide text-white/90">{t('myIp')}</text>
              <text testId="home-ip" className="text-xs font-mono text-white/90">{data.ipAddr}</text>
            </row>}
          </column>
        </scroll>
    </PageContent>
  </stack>;
}

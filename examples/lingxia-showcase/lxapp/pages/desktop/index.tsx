import { t } from '@lingxia/view';
import '../../theme/index.css';
import { Button, Card, Notice, Page } from '../../shared/ui';
import type { View } from './$types';

/** One thing only a desktop does: what it is, one line, one button. */
function Demo({ id, title, line, button, onClick, said }: {
  id: string;
  title: string;
  line: string;
  button: string;
  onClick: () => unknown;
  said?: string;
}) {
  return (
    <Card testId={`desktop-${id}`} className="p-4 gap-3">
      <view className="flex-row items-center gap-4">
        <view className="grow basis-0 min-w-0 gap-0.5">
          <text role="heading" headingLevel={2} className="text-sm font-semibold">{title}</text>
          <text className="text-xs text-muted line-clamp-2">{line}</text>
        </view>
        <Button size="sm" variant="secondary" testId={`desktop-${id}-open`} onClick={onClick} className="shrink-0">
          {button}
        </Button>
      </view>
      {said ? <Notice tone="success" testId={`desktop-${id}-said`}>{t('desktop.said', { message: said })}</Notice> : null}
    </Card>
  );
}

export function render({ data, actions, env }: View) {
  return (
    <Page testId="desktop-page" scrollId="desktop-scroll" intro={t('desktop.intro')}>
      {data.canWindow ? (
        <Demo id="window" title={t('desktop.window')} line={t('desktop.windowLine')} button={t('desktop.open')}
          onClick={actions.openWindow} said={data.windowMessage} />
      ) : null}
      <Demo id="float" title={t('desktop.float')} line={t('desktop.floatLine')} button={t('desktop.open')}
        onClick={actions.openFloat} said={data.floatMessage} />
      <Demo id="aside" title={t('desktop.aside')} line={t('desktop.asideLine')} button={t('desktop.open')}
        onClick={actions.openAside} />
      <Demo id="app" title={t('desktop.app')} line={t('desktop.appLine')} button={t('desktop.open')}
        onClick={actions.openShowcaseWeb} />
      {data.canTab ? (
        <Demo id="tab" title={t('desktop.tab')} line={t('desktop.tabLine')} button={t('desktop.open')}
          onClick={actions.openTab} />
      ) : null}
      <Demo id="terminal" title={t('desktop.terminal')} line={t('desktop.terminalLine')} button={t('desktop.open')}
        onClick={actions.openTerminal} />
      <Demo id="appearance" title={t('desktop.appearance')} line={t('desktop.appearanceLine')}
        button={env.appearance === 'dark' ? t('desktop.toLight') : t('desktop.toDark')}
        onClick={actions.switchAppearance} />
    </Page>
  );
}

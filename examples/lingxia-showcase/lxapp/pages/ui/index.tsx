import { t, useLocal, type Child, type LinkTarget, type Local, type MessageKey } from '@lingxia/view';
import '../../theme/index.css';
import { Button, Card, Field, InfoList, InfoRow, Page, Section, Segmented } from '../../shared/ui';
import type { View } from './$types';

type Data = Record<string, any>;
type Actions = Record<string, (payload?: unknown) => Promise<unknown>>;
type Pane = { data: Data; actions: Actions };

export function render({ data, actions }: View) {
  const pane = { data: data as Data, actions: actions as Actions };
  const type = String(data.currentType || 'navigation');
  if (['toast', 'actionsheet', 'modal'].includes(type)) return <Dialogs {...pane} />;
  if (type === 'appearance') return <Appearance {...pane} />;
  if (type === 'navbar') return <Navbar {...pane} />;
  if (type === 'tabbar') return <Tabbar {...pane} />;
  if (type === 'navigation') return <Navigation {...pane} />;
  return <Surface {...pane} />;
}

function ChromeError({ data }: { data: Data }) {
  return data.chromeError ? (
    <text data-testid="ui-chrome-error" className="px-3 py-2 rounded-lg border border-danger/40 bg-danger-soft text-sm text-danger break-all">
      {String(data.chromeError)}
    </text>
  ) : null;
}

/** A labeled value that opens a Logic picker. */
function Choice({ testId, label, value, onClick }: { testId: string; label: string; value: string; onClick: () => unknown }) {
  return (
    <view className="gap-1.5">
      <text className="text-sm font-medium">{label}</text>
      <button
        data-testid={testId}
        aria-label={`${label}: ${value}`}
        onClick={() => {
          void onClick();
        }}
        className="px-3 py-2 rounded-lg border border-line-strong bg-surface text-text active:bg-surface-sunken"
      >
        <view className="flex-row items-center justify-between gap-3">
          <text className="text-base">{value}</text>
          <text className="text-xs text-accent">{t('ui.change')}</text>
        </view>
      </button>
    </view>
  );
}

function Dialogs({ data, actions }: Pane) {
  const type = String(data.currentType);
  const toastTitle = useLocal('toastTitle', t('ui.helloToast'));
  const toastDuration = useLocal('toastDuration', '2000');
  const modalTitle = useLocal('modalTitle', 'Alert');
  const modalContent = useLocal('modalContent', t('ui.modalContent'));
  const modalCancel = useLocal('modalCancel', true);
  const cancelText = useLocal('cancelText', t('common.cancel'));
  const confirmText = useLocal('confirmText', t('common.ok'));
  const showToast = () =>
    actions.showToastWithParams({
      message: toastTitle.value,
      durationMs: parseInt(toastDuration.value, 10) || 2000,
    });
  const showModal = () =>
    actions.showModalWithParams({
      title: modalTitle.value,
      content: modalContent.value,
      showCancel: modalCancel.value,
      cancelText: cancelText.value,
      confirmText: confirmText.value,
    });
  return (
    <Page testId="ui-page" title={data.navTitle ?? undefined} data={{ 'data-instance-tag': String(data.instanceTag ?? '') }}>
      <ChromeError data={data} />
      {type === 'toast' && (
        <Section title={t('ui.toastParameters')}>
          <Field testId="toast-title" label={t('ui.message')} bind={toastTitle} placeholder={t('ui.enterToastMessage')} />
          <Choice testId="toast-style" label={t('ui.style')} value={t(String(data.toastStyleLabel || 'ui.toast.success') as MessageKey)} onClick={() => actions.chooseToastStyle()} />
          <Field testId="toast-duration" label={t('ui.durationMs')} bind={toastDuration} type="number" />
          <Choice
            testId="toast-position"
            label={t('ui.position')}
            value={t(String(data.toastPositionLabel || 'ui.toast.center') as MessageKey)}
            onClick={() => actions.chooseToastPosition()}
          />
          <view className="grid gap-3 grid-cols-2">
            <Button testId="toast-show" onClick={showToast}>
              {t('ui.showToast')}
            </Button>
            <Button variant="secondary" testId="toast-hide" onClick={() => actions.hideToast()}>
              {t('ui.hideToast')}
            </Button>
          </view>
        </Section>
      )}
      {type === 'actionsheet' && (
        <Section title={t('ui.actionSheet')}>
          <Button testId="actionsheet-show" onClick={() => actions.showDemoActionSheet()}>
            {t('ui.showActionSheet')}
          </Button>
          {!!data.actionSheetResult && (
            <text data-testid="actionsheet-result" className="text-xs font-mono break-all whitespace-pre-wrap">
              {JSON.stringify(data.actionSheetResult, null, 2)}
            </text>
          )}
        </Section>
      )}
      {type === 'modal' && (
        <Section title={t('ui.modalParameters')}>
          <Field testId="modal-title" label={t('ui.titleOptional')} bind={modalTitle} placeholder={t('ui.leaveEmptyForNo')} />
          <view className="gap-1.5">
            <text className="text-sm font-medium">{t('ui.content')}</text>
            <textarea
              data-testid="modal-content"
              aria-label={t('ui.content')}
              bind={modalContent}
              placeholder={t('ui.enterModalContent')}
              className="h-24 px-3 py-2 rounded-lg border border-line-strong bg-surface text-base"
            />
          </view>
          <checkbox data-testid="modal-show-cancel" bind={modalCancel} className="text-sm">
            {t('ui.showCancelButton')}
          </checkbox>
          <Field testId="modal-cancel-text" label={t('ui.cancelButtonText')} bind={cancelText} placeholder={t('ui.cancelButtonText2')} />
          <Field testId="modal-confirm-text" label={t('ui.confirmButtonText')} bind={confirmText} placeholder={t('ui.confirmButtonText2')} />
          <Button testId="modal-show" onClick={showModal}>
            {t('ui.showModal')}
          </Button>
        </Section>
      )}
      {type === 'modal' && !!data.modalResult && (
        <Section title={t('ui.modalResult')}>
          <text data-testid="modal-result" className="text-xs font-mono text-muted break-all whitespace-pre-wrap">
            {JSON.stringify(data.modalResult, null, 2)}
          </text>
          <Button variant="ghost" testId="modal-clear" onClick={() => actions.clearModalResult()}>
            {t('ui.clearResult')}
          </Button>
        </Section>
      )}
    </Page>
  );
}

function Appearance({ data, actions }: Pane) {
  const appearance = data.appearance as { preference: string; resolved: string } | undefined;
  const preference = appearance?.preference || 'auto';
  return (
    <Page testId="ui-page" title={data.navTitle ?? undefined} data={{ 'data-instance-tag': String(data.instanceTag ?? '') }}>
      <ChromeError data={data} />
      <Section
        title={t('ui.lightDark')}
        description={t('ui.autoFollowsTheHost')}
      >
        <Segmented
          testId="ui-appearance"
          value={preference}
          onChange={value => actions.setAppearance({ preference: value })}
          options={[
            { value: 'auto', label: t('ui.theme.auto') },
            { value: 'light', label: t('ui.theme.light') },
            { value: 'dark', label: t('ui.theme.dark') },
          ]}
        />
        <InfoList>
          <InfoRow label={t('ui.preference')} value={preference} testId="ui-appearance-preference" />
          <InfoRow label={t('ui.resolved')} value={appearance?.resolved} testId="ui-appearance-resolved" />
        </InfoList>
        <text className="text-xs text-muted">
          {t('ui.theResolvedThemeIs')}
        </text>
      </Section>
    </Page>
  );
}

function Navbar({ data, actions }: Pane) {
  const title = useLocal('navTitle', '');
  const background = useLocal('navBackground', '');
  const foreground = useLocal('navForeground', '');
  const preset = (name: string, backgroundColor: string, frontColor: string) => (
    <Button
      variant="secondary"
      size="sm"
      key={name}
      testId={`navbar-preset-${name.toLowerCase()}`}
      onClick={async () => {
        await actions.updateNavigationBarTitle({ title: `${name} Theme` });
        await actions.updateNavigationBarColors({ backgroundColor, frontColor });
      }}
    >
      {name}
    </Button>
  );
  return (
    <Page testId="ui-page" title={data.navTitle ?? undefined} data={{ 'data-instance-tag': String(data.instanceTag ?? '') }}>
      <ChromeError data={data} />
      <Section title={t('ui.navigationbarApis')}>
        <Field testId="navbar-title-input" label={t('ui.title')} bind={title} placeholder={t('ui.enterTitle')} />
        <Button testId="navbar-set-title" onClick={() => title.value && actions.updateNavigationBarTitle({ title: title.value })}>
          {t('ui.setTitle')}
        </Button>
        <view className="grid gap-2 grid-cols-2">
          <Field testId="navbar-bg-input" label={t('ui.background')} bind={background} placeholder={t('ui.ffffff')} />
          <Field testId="navbar-fg-input" label={t('ui.text')} bind={foreground} placeholder="#000000" />
        </view>
        <Button
          testId="navbar-set-colors"
          onClick={() =>
            actions.updateNavigationBarColors({
              backgroundColor: background.value || '#ffffff',
              frontColor: foreground.value || '#000000',
            })
          }
        >
          {t('ui.setColors')}
        </Button>
        <view className="grid gap-2 grid-cols-2">
          {preset('Dark', '#1f2937', '#ffffff')}
          {preset('Blue', '#3b82f6', '#ffffff')}
          {preset('Light', '#ffffff', '#000000')}
          {preset('Green', '#10b981', '#ffffff')}
        </view>
        <view className="grid gap-2 grid-cols-3">
          <Button
            variant="secondary"
            size="sm"
            testId="navbar-home-hide"
            onClick={() => actions.updateNavigationBarHomeButton({ homeButton: 'hidden' })}
          >
            {t('ui.hideHome')}
          </Button>
          <Button
            variant="secondary"
            size="sm"
            testId="navbar-home-auto"
            onClick={() => actions.updateNavigationBarHomeButton({ homeButton: 'auto' })}
          >
            {t('ui.autoHome')}
          </Button>
          <Button variant="ghost" size="sm" testId="navbar-reset" onClick={() => actions.resetNavigationBar()}>
            {t('ui.reset')}
          </Button>
        </view>
      </Section>
    </Page>
  );
}

function Tabbar({ data, actions }: Pane) {
  const itemText = useLocal('itemText', t('ui.newTab'));
  const badge = useLocal('badge', '99');
  const pair = (left: Child, right: Child) => (
    <view className="grid gap-3 grid-cols-2">
      {left}
      {right}
    </view>
  );
  return (
    <Page testId="ui-page" title={data.navTitle ?? undefined} data={{ 'data-instance-tag': String(data.instanceTag ?? '') }}>
      <ChromeError data={data} />
      <Section title={t('ui.visibilityControls')} description={t('ui.showOrHideThe')}>
        {pair(
          <Button testId="tabbar-show" onClick={() => actions.revealTabBar()}>
            {t('ui.showTabbar')}
          </Button>,
          <Button variant="secondary" testId="tabbar-hide" onClick={() => actions.concealTabBar()}>
            {t('ui.hideTabbar')}
          </Button>,
        )}
        <Field testId="tabbar-item-text" label={t('ui.updateTab1Text')} bind={itemText} placeholder={t('ui.enterNewText')} />
        <Button testId="tabbar-item-update" onClick={() => actions.updateTabBarItem({ tab: 'api', label: itemText.value })}>
          {t('ui.update')}
        </Button>
      </Section>
      <Section title={t('ui.redDotControls')} description={t('ui.showOrHideThe2')}>
        {pair(
          <Button variant="destructive" testId="tabbar-reddot-show" onClick={() => actions.enableTabBarRedDot({ tab: 'api' })}>
            {t('ui.showRedDot')}
          </Button>,
          <Button variant="secondary" testId="tabbar-reddot-hide" onClick={() => actions.disableTabBarRedDot({ tab: 'api' })}>
            {t('ui.hideRedDot')}
          </Button>,
        )}
      </Section>
      <Section title={t('ui.badgeControls')} description={t('ui.setOrRemoveThe')}>
        <Field testId="tabbar-badge-input" label={t('ui.badgeText')} bind={badge} placeholder={t('ui.enterBadgeText')} />
        {pair(
          <Button testId="tabbar-badge-set" onClick={() => actions.updateTabBarBadge({ tab: 'api', text: badge.value })}>
            {t('ui.setBadge')}
          </Button>,
          <Button variant="secondary" testId="tabbar-badge-clear" onClick={() => actions.clearTabBarBadge({ tab: 'api' })}>
            {t('ui.removeBadge')}
          </Button>,
        )}
      </Section>
    </Page>
  );
}

// A row that navigates: `to` for a plain jump, `onClick` when Logic has to decide.
function NavLink({ testId, label, description, to, onClick }: { testId: string; label: string; description?: string; to?: LinkTarget; onClick?: () => void }) {
  return (
    <link
      data-testid={testId}
      to={to}
      onClick={onClick}
      className="px-4 py-3.5 rounded-none bg-transparent text-text text-left active:bg-surface-sunken"
    >
      <view className="flex-row items-center justify-between gap-3">
        <view className="grow basis-0 gap-1">
          <text className="text-sm font-medium">{label}</text>
          {!!description && <text className="text-xs text-muted">{description}</text>}
        </view>
        <text className="text-lg text-muted">›</text>
      </view>
    </link>
  );
}

function Counter({ id, label, value, hint, onClick }: { id: string; label: string; value: number; hint: string; onClick: () => unknown }) {
  return (
    <button
      data-testid={`lifecycle-bump-${id}`}
      aria-label={`${label}: ${value}`}
      onClick={() => {
        void onClick();
      }}
      className="px-2 py-4 rounded-xl bg-surface-sunken text-text active:bg-line"
    >
      <view className="items-center gap-1">
        <text data-testid={`lifecycle-${id}-counter`} className="text-2xl font-bold">
          {String(value)}
        </text>
        <text className="text-sm font-medium">{label}</text>
        <text className="text-[10px] text-muted">{hint}</text>
      </view>
    </button>
  );
}

function Navigation({ data, actions }: Pane) {
  const count = useLocal('viewCounter', 0);
  const popup = useLocal('popup', false);
  const events = (data.events || []) as string[];
  return (
    <page data-testid="ui-page" data-instance-tag={String(data.instanceTag ?? '')} className="bg-canvas text-text">
      <scroll data-testid="page-scroll" className="grow basis-0">
        <view className="w-full max-w-3xl self-center px-4 pt-5 pb-10 gap-4">
          <ChromeError data={data} />
          <Card className="divide-y divide-line">
            <NavLink
              testId="ui-navigate-to"
              label={t('ui.pushThisPageAgain')}
              description={t('ui.eachEntryCreatesA')}
              onClick={() => void actions.demoNavigateTo()}
            />
            <NavLink testId="ui-navigate-back" label={t('ui.backToPreviousPage')} to={{ back: 1 }} />
            <NavLink testId="ui-redirect-to" label={t('ui.replaceThisPageRedirectto')} to={{ page: 'ui', params: { type: 'navigation' }, replace: true }} />
            <NavLink testId="ui-switch-tab" label={t('ui.jumpToTabPage')} to={{ tab: 'home' }} />
          </Card>
          <Section title={t('ui.logicInstance')} description={t('ui.pushThisPageAgain2')}>
            <InfoList>
              <InfoRow label={t('ui.thisInstance')} value={`#${data.instanceTag || '-'}`} testId="lifecycle-instance-tag" />
              <InfoRow label={t('ui.previousInstance')} value={`#${data.previousInstanceTag || '-'}`} testId="lifecycle-previous-tag" />
            </InfoList>
            <text className="text-[11px] text-muted">{t('ui.thePreviousTagIs')}</text>
          </Section>
          <Section
            title={t('ui.stateThatResetsAnd')}
            description={t('ui.logicAndViewReset')}
          >
            <view className="grid gap-3 grid-cols-3">
              <Counter
                id="logic"
                label={t('ui.logic1')}
                value={Number(data.logicCounter || 0)}
                hint={t('ui.thisSetdata')}
                onClick={() => actions.bumpLogicCounter()}
              />
              <Counter id="view" label={t('ui.view1')} value={count.value} hint={t('ui.uselocal')} onClick={() => count.set(count.value + 1)} />
              <Counter
                id="module"
                label={t('ui.module1')}
                value={Number(data.moduleCounter || 0)}
                hint={t('ui.moduleScope')}
                onClick={() => actions.bumpModuleCounter()}
              />
            </view>
            <Button testId="lifecycle-open-popup" onClick={() => popup.set(true)}>
              {t('ui.openViewPopup')}
            </Button>
          </Section>
          <Section title={t('ui.lifecycleOfThisInstance')} description={t('ui.storedInPageData')}>
            <view data-testid="lifecycle-events" className="gap-2">
              {events.map((event, index) => (
                <text key={String(index)} className="px-3 py-2 rounded-lg border border-line bg-surface-sunken text-xs font-mono break-all">
                  {event}
                </text>
              ))}
              {!events.length && <text className="text-xs text-muted">{t('ui.noEventsYet')}</text>}
            </view>
            <text className="text-[11px] text-muted">
              {t('ui.backgroundingFiresOnhideOnshow')}
            </text>
          </Section>
        </view>
      </scroll>
      {popup.value && (
        <view data-testid="lifecycle-popup" className="absolute inset-0 z-10 items-center-safe justify-center-safe px-8">
          <button
            data-testid="lifecycle-popup-backdrop"
            aria-label={t('ui.closePopup')}
            onClick={() => popup.set(false)}
            className="absolute inset-0 rounded-none bg-scrim"
          />
          <Card className="w-full max-w-xs px-5 py-6 gap-4 shadow-xl">
            <text className="text-base font-semibold text-center">{t('ui.viewPopup')}</text>
            <text className="text-xs text-muted text-center">
              {t('ui.leaveThisPageWith')}
            </text>
            <Button variant="secondary" testId="lifecycle-popup-close" onClick={() => popup.set(false)}>
              {t('ui.close')}
            </Button>
          </Card>
        </view>
      )}
    </page>
  );
}

const kinds = () =>
  ['aside', 'float', 'window', 'lxapp'].map(value => ({ value, label: t(`ui.kind.${value}` as MessageKey), hint: t(`ui.kindHint.${value}` as MessageKey) }));
const titled = (values: string[]) => values.map(value => ({ value, label: t(`ui.option.${value}` as MessageKey) }));

function parseSize(raw: string, label: string): number | string | undefined {
  const value = raw.trim();
  if (!value) return undefined;
  const percent = value.endsWith('%');
  const amount = Number(percent ? value.slice(0, -1).trim() : value);
  if (!Number.isFinite(amount) || amount <= 0 || (percent && amount > 100))
    throw new Error(
      percent ? t('ui.sizePercent', { label }) : t('ui.sizePx', { label }),
    );
  return percent ? `${amount}%` : amount;
}

function Options({
  label,
  testId,
  bind,
  options,
  disabled,
}: {
  label: string;
  testId: string;
  bind: Local<string>;
  options: { value: string; label: string }[];
  disabled?: boolean;
}) {
  return (
    <view className="gap-2">
      <text className="text-xs text-muted uppercase">{label}</text>
      <view className={options.length > 3 ? 'grid gap-2 grid-cols-2' : 'grid gap-2 grid-cols-3'}>
        {options.map(option => (
          <Button
            key={option.value}
            size="sm"
            variant={bind.value === option.value ? 'primary' : 'secondary'}
            testId={`${testId}-${option.value}`}
            disabled={disabled}
            onClick={() => bind.set(option.value)}
          >
            {option.label}
          </Button>
        ))}
      </view>
    </view>
  );
}

function Surface({ data, actions }: Pane) {
  const kind = useLocal('surfaceKind', 'aside');
  const edge = useLocal('surfaceEdge', 'right');
  const position = useLocal('surfacePosition', 'center');
  const chrome = useLocal('surfaceChrome', 'full');
  const width = useLocal('surfaceWidth', '');
  const height = useLocal('surfaceHeight', '');
  const attempted = useLocal('surfaceSizeAttempted', false);
  const state = data.surfaceDemo as { active?: boolean; visible?: boolean; message?: string } | undefined;
  const active = state?.active === true;
  let sizeError = '';
  let size: { width?: number | string; height?: number | string } = {};
  try {
    const w = parseSize(width.value, t('ui.width')),
      h = parseSize(height.value, t('ui.height'));
    size = { ...(w === undefined ? {} : { width: w }), ...(h === undefined ? {} : { height: h }) };
  } catch (error) {
    sizeError = error instanceof Error ? error.message : String(error);
  }
  const open = () => {
    attempted.set(true);
    if (!sizeError)
      void actions.openSurfaceDemo({
        verb: kind.value,
        edge: edge.value,
        position: position.value,
        ...(kind.value === 'window' ? { chrome: chrome.value } : {}),
        ...size,
      });
  };
  return (
    <Page testId="ui-page" title={data.navTitle ?? undefined} data={{ 'data-instance-tag': String(data.instanceTag ?? '') }}>
      <Section title={t('ui.openASurface')} description={kinds().find(item => item.value === kind.value)?.hint}>
        <Options label={t('ui.kind')} testId="surface-kind" bind={kind} options={kinds()} disabled={active} />
        {kind.value === 'aside' && <Options label={t('ui.edge')} testId="surface-edge" bind={edge} options={titled(['left', 'right', 'top', 'bottom'])} />}
        {kind.value === 'float' && (
          <Options label={t('ui.position')} testId="surface-position" bind={position} options={titled(['center', 'top', 'bottom', 'left', 'right'])} />
        )}
        {kind.value === 'window' && <Options label={t('ui.chrome')} testId="surface-chrome" bind={chrome} options={titled(['system', 'full'])} />}
        <view className="grid gap-2 grid-cols-2">
          <Field testId="surface-width" label={t('ui.widthPxOr')} bind={width} placeholder={t('ui.widthPxOr2')} />
          <Field testId="surface-height" label={t('ui.heightPxOr')} bind={height} placeholder={t('ui.heightPxOr2')} />
        </view>
        {attempted.value && !!sizeError && (
          <text data-testid="size-error" className="text-xs text-danger">
            {sizeError}
          </text>
        )}
        <Button testId="open-surface" data={{ 'data-surface-width': width.value, 'data-surface-height': height.value }} disabled={active} onClick={open}>{`Open ${kind.value}${active ? ' (already open)' : ''}`}</Button>
        <text className="text-xs text-muted">
          Placement applies when the surface opens; hiding and showing keeps it. Close and reopen to change it.
        </text>
        {active && (
          <view className="grid gap-2 grid-cols-3">
            <Button size="sm" testId="surface-parent-show" disabled={!!state?.visible} onClick={() => actions.showActiveSurface()}>
              {t('ui.show')}
            </Button>
            <Button size="sm" variant="secondary" testId="surface-parent-hide" disabled={!state?.visible} onClick={() => actions.hideActiveSurface()}>
              {t('ui.hide')}
            </Button>
            <Button size="sm" variant="destructive" testId="surface-parent-close" onClick={() => actions.closeActiveSurface()}>
              {t('ui.close')}
            </Button>
          </view>
        )}
        <text data-testid="surface-status" className="px-3 py-2 rounded-lg bg-surface-sunken text-sm font-mono break-all">
          {state?.message || t('ui.noMessage')}
        </text>
      </Section>
    </Page>
  );
}

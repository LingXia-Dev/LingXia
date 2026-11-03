import { useLocal, type Child, type Local, type View } from '@lingxia/view';
import '../../theme/index.css';
import { Button, Card, Field, InfoList, InfoRow, Page, Section, Segmented } from '../../shared/ui';

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
    <column className="gap-1.5">
      <text className="text-sm font-medium">{label}</text>
      <button
        data-testid={testId}
        aria-label={`${label}: ${value}`}
        onClick={() => {
          void onClick();
        }}
        className="px-3 py-2 rounded-lg border border-line-strong bg-surface text-text active:bg-surface-sunken"
      >
        <row className="items-center justify-between gap-3">
          <text className="text-base">{value}</text>
          <text className="text-xs text-accent">Change</text>
        </row>
      </button>
    </column>
  );
}

function Dialogs({ data, actions }: Pane) {
  const type = String(data.currentType);
  const toastTitle = useLocal('toastTitle', 'Hello Toast!');
  const toastDuration = useLocal('toastDuration', '2000');
  const toastMask = useLocal('toastMask', false);
  const modalTitle = useLocal('modalTitle', 'Alert');
  const modalContent = useLocal('modalContent', 'This is a modal dialog');
  const modalCancel = useLocal('modalCancel', true);
  const cancelText = useLocal('cancelText', 'Cancel');
  const confirmText = useLocal('confirmText', 'OK');
  const showToast = () =>
    actions.showToastWithParams({
      title: toastTitle.value,
      icon: data.toastIcon,
      durationMs: parseInt(toastDuration.value, 10) || 2000,
      position: data.toastPosition,
      mask: toastMask.value,
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
    <Page testId="ui-page" data={{ 'data-instance-tag': String(data.instanceTag ?? '') }}>
      <ChromeError data={data} />
      {type === 'toast' && (
        <Section title="Toast Parameters">
          <Field testId="toast-title" label="Title" bind={toastTitle} placeholder="Enter toast title" />
          <Choice testId="toast-icon" label="Icon" value={String(data.toastIconLabel || 'Success')} onClick={() => actions.chooseToastIcon()} />
          <Field testId="toast-duration" label="Duration (ms)" bind={toastDuration} type="number" />
          <Choice
            testId="toast-position"
            label="Position"
            value={String(data.toastPositionLabel || 'Center')}
            onClick={() => actions.chooseToastPosition()}
          />
          <checkbox data-testid="toast-mask" bind={toastMask} className="text-sm">
            Show mask (prevents interaction)
          </checkbox>
          <grid className="gap-3 grid-cols-2">
            <Button testId="toast-show" onClick={showToast}>
              Show Toast
            </Button>
            <Button variant="secondary" testId="toast-hide" onClick={() => actions.hideToast()}>
              Hide Toast
            </Button>
          </grid>
        </Section>
      )}
      {type === 'actionsheet' && (
        <Section title="Action Sheet">
          <Button testId="actionsheet-show" onClick={() => actions.showDemoActionSheet()}>
            Show Action Sheet
          </Button>
          {!!data.actionSheetResult && (
            <text data-testid="actionsheet-result" className="text-xs font-mono break-all whitespace-pre-wrap">
              {JSON.stringify(data.actionSheetResult, null, 2)}
            </text>
          )}
        </Section>
      )}
      {type === 'modal' && (
        <Section title="Modal Parameters">
          <Field testId="modal-title" label="Title (optional)" bind={modalTitle} placeholder="Leave empty for no title" />
          <column className="gap-1.5">
            <text className="text-sm font-medium">Content</text>
            <textarea
              data-testid="modal-content"
              aria-label="Content"
              bind={modalContent}
              placeholder="Enter modal content"
              className="h-24 px-3 py-2 rounded-lg border border-line-strong bg-surface text-base"
            />
          </column>
          <checkbox data-testid="modal-show-cancel" bind={modalCancel} className="text-sm">
            Show cancel button
          </checkbox>
          <Field testId="modal-cancel-text" label="Cancel Button Text" bind={cancelText} placeholder="Cancel button text" />
          <Field testId="modal-confirm-text" label="Confirm Button Text" bind={confirmText} placeholder="Confirm button text" />
          <Button testId="modal-show" onClick={showModal}>
            Show Modal
          </Button>
        </Section>
      )}
      {type === 'modal' && !!data.modalResult && (
        <Section title="Modal Result">
          <text data-testid="modal-result" className="text-xs font-mono text-muted break-all whitespace-pre-wrap">
            {JSON.stringify(data.modalResult, null, 2)}
          </text>
          <Button variant="ghost" testId="modal-clear" onClick={() => actions.clearModalResult()}>
            Clear Result
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
    <Page testId="ui-page" data={{ 'data-instance-tag': String(data.instanceTag ?? '') }}>
      <ChromeError data={data} />
      <Section
        title="Light / Dark"
        description="Auto follows the host appearance; Light and Dark keep your chosen theme. Your preference is saved for this app."
      >
        <Segmented
          testId="ui-appearance"
          value={preference}
          onChange={value => actions.setAppearance({ preference: value })}
          options={[
            { value: 'auto', label: 'Auto' },
            { value: 'light', label: 'Light' },
            { value: 'dark', label: 'Dark' },
          ]}
        />
        <InfoList>
          <InfoRow label="Preference" value={preference} testId="ui-appearance-preference" />
          <InfoRow label="Resolved" value={appearance?.resolved} testId="ui-appearance-resolved" />
        </InfoList>
        <text className="text-xs text-muted">
          The resolved theme is in use now. Pages share one palette, so their colors update together.
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
    <Page testId="ui-page" data={{ 'data-instance-tag': String(data.instanceTag ?? '') }}>
      <ChromeError data={data} />
      <Section title="NavigationBar APIs">
        <Field testId="navbar-title-input" label="Title" bind={title} placeholder="Enter title" />
        <Button testId="navbar-set-title" onClick={() => title.value && actions.updateNavigationBarTitle({ title: title.value })}>
          Set Title
        </Button>
        <grid className="gap-2 grid-cols-2">
          <Field testId="navbar-bg-input" label="Background" bind={background} placeholder="#ffffff" />
          <Field testId="navbar-fg-input" label="Text" bind={foreground} placeholder="#000000" />
        </grid>
        <Button
          testId="navbar-set-colors"
          onClick={() =>
            actions.updateNavigationBarColors({
              backgroundColor: background.value || '#ffffff',
              frontColor: foreground.value || '#000000',
            })
          }
        >
          Set Colors
        </Button>
        <grid className="gap-2 grid-cols-2">
          {preset('Dark', '#1f2937', '#ffffff')}
          {preset('Blue', '#3b82f6', '#ffffff')}
          {preset('Light', '#ffffff', '#000000')}
          {preset('Green', '#10b981', '#ffffff')}
        </grid>
        <grid className="gap-2 grid-cols-3">
          <Button
            variant="secondary"
            size="sm"
            testId="navbar-home-hide"
            onClick={() => actions.updateNavigationBarHomeButton({ homeButton: 'hidden' })}
          >
            Hide home
          </Button>
          <Button
            variant="secondary"
            size="sm"
            testId="navbar-home-auto"
            onClick={() => actions.updateNavigationBarHomeButton({ homeButton: 'auto' })}
          >
            Auto home
          </Button>
          <Button variant="ghost" size="sm" testId="navbar-reset" onClick={() => actions.resetNavigationBar()}>
            Reset
          </Button>
        </grid>
      </Section>
    </Page>
  );
}

function Tabbar({ data, actions }: Pane) {
  const itemText = useLocal('itemText', 'New Tab');
  const badge = useLocal('badge', '99');
  const pair = (left: Child, right: Child) => (
    <grid className="gap-3 grid-cols-2">
      {left}
      {right}
    </grid>
  );
  return (
    <Page testId="ui-page" data={{ 'data-instance-tag': String(data.instanceTag ?? '') }}>
      <ChromeError data={data} />
      <Section title="Visibility Controls" description="Show or hide the TabBar and update item text">
        {pair(
          <Button testId="tabbar-show" onClick={() => actions.revealTabBar()}>
            Show TabBar
          </Button>,
          <Button variant="secondary" testId="tabbar-hide" onClick={() => actions.concealTabBar()}>
            Hide TabBar
          </Button>,
        )}
        <Field testId="tabbar-item-text" label="Update Tab 1 Text" bind={itemText} placeholder="Enter new text" />
        <Button testId="tabbar-item-update" onClick={() => actions.updateTabBarItem({ index: 1, text: itemText.value })}>
          Update
        </Button>
      </Section>
      <Section title="Red Dot Controls" description="Show or hide the red dot on tab 1">
        {pair(
          <Button variant="destructive" testId="tabbar-reddot-show" onClick={() => actions.enableTabBarRedDot({ index: 1 })}>
            Show Red Dot
          </Button>,
          <Button variant="secondary" testId="tabbar-reddot-hide" onClick={() => actions.disableTabBarRedDot({ index: 1 })}>
            Hide Red Dot
          </Button>,
        )}
      </Section>
      <Section title="Badge Controls" description="Set or remove the badge on tab 1">
        <Field testId="tabbar-badge-input" label="Badge Text" bind={badge} placeholder="Enter badge text" />
        {pair(
          <Button testId="tabbar-badge-set" onClick={() => actions.updateTabBarBadge({ index: 1, text: badge.value })}>
            Set Badge
          </Button>,
          <Button variant="secondary" testId="tabbar-badge-clear" onClick={() => actions.clearTabBarBadge({ index: 1 })}>
            Remove Badge
          </Button>,
        )}
      </Section>
    </Page>
  );
}

function NavLink({ testId, label, description, onClick }: { testId: string; label: string; description?: string; onClick: () => unknown }) {
  return (
    <button
      data-testid={testId}
      onClick={() => {
        void onClick();
      }}
      className="px-4 py-3.5 rounded-none bg-transparent text-text text-left active:bg-surface-sunken"
    >
      <row className="items-center justify-between gap-3">
        <column className="grow basis-0 gap-1">
          <text className="text-sm font-medium">{label}</text>
          {!!description && <text className="text-xs text-muted">{description}</text>}
        </column>
        <text className="text-lg text-muted">›</text>
      </row>
    </button>
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
      <column className="items-center gap-1">
        <text data-testid={`lifecycle-${id}-counter`} className="text-2xl font-bold">
          {String(value)}
        </text>
        <text className="text-sm font-medium">{label}</text>
        <text className="text-[10px] text-muted">{hint}</text>
      </column>
    </button>
  );
}

function Navigation({ data, actions }: Pane) {
  const count = useLocal('viewCounter', 0);
  const popup = useLocal('popup', false);
  const pages = (data.pageStack || []) as { index: number; name: string; current: boolean }[];
  const events = (data.events || []) as string[];
  return (
    <page data-testid="ui-page" data-instance-tag={String(data.instanceTag ?? '')} className="bg-canvas text-text">
      <scroll data-testid="page-scroll" className="grow basis-0">
        <column className="w-full max-w-3xl self-center px-4 pt-5 pb-10 gap-4">
          <ChromeError data={data} />
          <Card className="divide-y divide-line">
            <NavLink
              testId="ui-navigate-to"
              label="Push this page again"
              description="Each entry creates a fresh instance. Watch the stack grow."
              onClick={() => actions.demoNavigateTo()}
            />
            <NavLink testId="ui-navigate-back" label="Back to previous page" onClick={() => actions.demoNavigateBack()} />
            <NavLink testId="ui-redirect-to" label="Replace this page (redirectTo)" onClick={() => actions.demoRedirectTo()} />
            <NavLink testId="ui-switch-tab" label="Jump to Tab page" onClick={() => actions.demoSwitchTab()} />
          </Card>
          <Section
            title="Page stack"
            description="getCurrentPages(), top is where you are"
            action={
              <text
                data-testid="ui-page-stack-depth"
                className="px-2.5 py-1 rounded-full bg-accent-soft text-xs font-mono text-accent"
              >{`${pages.length}/10`}</text>
            }
          >
            <column data-testid="ui-page-stack-list" className="gap-1.5">
              {[...pages].reverse().map(page => (
                <row
                  key={String(page.index)}
                  className={
                    page.current
                      ? 'items-center gap-3 px-3 py-2 rounded-lg border border-line bg-accent-soft'
                      : 'items-center gap-3 px-3 py-2 rounded-lg border border-line bg-surface-sunken'
                  }
                >
                  <text className="w-5 text-right text-xs font-mono text-muted">{String(page.index + 1)}</text>
                  <text className="grow basis-0 text-xs font-mono">{page.name}</text>
                  {page.current && <text className="text-[10px] font-medium">you are here</text>}
                </row>
              ))}
              {!pages.length && <text className="py-4 text-xs text-muted text-center">No page stack available</text>}
            </column>
          </Section>
          <Section title="Logic instance" description="Push this page again to create a new Page instance with a new tag.">
            <InfoList>
              <InfoRow label="This instance" value={`#${data.instanceTag || '-'}`} testId="lifecycle-instance-tag" />
              <InfoRow label="Previous instance" value={`#${data.previousInstanceTag || '-'}`} testId="lifecycle-previous-tag" />
            </InfoList>
            <text className="text-[11px] text-muted">The previous tag is saved in app storage and survives a page reset.</text>
          </Section>
          <Section
            title="State that resets and state that stays"
            description="Logic and View reset per entry; the module counter is shared by every instance."
          >
            <grid className="gap-3 grid-cols-3">
              <Counter
                id="logic"
                label="Logic +1"
                value={Number(data.logicCounter || 0)}
                hint="this.setData"
                onClick={() => actions.bumpLogicCounter()}
              />
              <Counter id="view" label="View +1" value={count.value} hint="useLocal" onClick={() => count.set(count.value + 1)} />
              <Counter
                id="module"
                label="Module +1"
                value={Number(data.moduleCounter || 0)}
                hint="module scope"
                onClick={() => actions.bumpModuleCounter()}
              />
            </grid>
            <Button testId="lifecycle-open-popup" onClick={() => popup.set(true)}>
              Open View popup
            </Button>
          </Section>
          <Section title="Lifecycle of this instance" description="Stored in page data, so it starts over with every entry.">
            <column data-testid="lifecycle-events" className="gap-2">
              {events.map((event, index) => (
                <text key={String(index)} className="px-3 py-2 rounded-lg border border-line bg-surface-sunken text-xs font-mono break-all">
                  {event}
                </text>
              ))}
              {!events.length && <text className="text-xs text-muted">No events yet</text>}
            </column>
            <text className="text-[11px] text-muted">
              Backgrounding fires onHide / onShow on the same instance. Leaving the page fires onUnload.
            </text>
          </Section>
        </column>
      </scroll>
      {popup.value && (
        <column data-testid="lifecycle-popup" className="absolute inset-0 z-10 items-center justify-center px-8">
          <button
            data-testid="lifecycle-popup-backdrop"
            aria-label="Close popup"
            onClick={() => popup.set(false)}
            className="absolute inset-0 rounded-none bg-scrim"
          />
          <Card className="w-full max-w-xs px-5 py-6 gap-4 shadow-xl">
            <text className="text-base font-semibold text-center">View popup</text>
            <text className="text-xs text-muted text-center">
              Leave this page with the popup open. A fresh visit starts with it closed, because View state belongs to each page instance.
            </text>
            <Button variant="secondary" testId="lifecycle-popup-close" onClick={() => popup.set(false)}>
              Close
            </Button>
          </Card>
        </column>
      )}
    </page>
  );
}

const kinds = [
  { value: 'aside', label: 'Aside', hint: 'Docks beside the main page. On compact screens, switch between tabs to view it.' },
  { value: 'float', label: 'Float', hint: 'A popup above the main page, like a dialog. It does not take layout space.' },
  { value: 'window', label: 'Window', hint: 'A standalone window with no sidebar or shell. Desktop only.' },
  { value: 'lxapp', label: 'Lxapp', hint: 'Opens the chat app as an aside. Requires home-app privilege.' },
];
const titled = (values: string[]) => values.map(value => ({ value, label: value[0].toUpperCase() + value.slice(1) }));

function parseSize(raw: string, label: string): number | string | undefined {
  const value = raw.trim();
  if (!value) return undefined;
  const percent = value.endsWith('%');
  const amount = Number(percent ? value.slice(0, -1).trim() : value);
  if (!Number.isFinite(amount) || amount <= 0 || (percent && amount > 100))
    throw new Error(
      `${label} must be ${percent ? 'a percentage greater than 0% and at most 100%' : 'a positive px value or a percentage like 80%'}.`,
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
    <column className="gap-2">
      <text className="text-xs text-muted uppercase">{label}</text>
      <grid className={options.length > 3 ? 'gap-2 grid-cols-2' : 'gap-2 grid-cols-3'}>
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
      </grid>
    </column>
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
    const w = parseSize(width.value, 'Width'),
      h = parseSize(height.value, 'Height');
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
    <Page testId="ui-page" data={{ 'data-instance-tag': String(data.instanceTag ?? '') }}>
      <Section title="Open a surface" description={kinds.find(item => item.value === kind.value)?.hint}>
        <Options label="Kind" testId="surface-kind" bind={kind} options={kinds} disabled={active} />
        {kind.value === 'aside' && <Options label="Edge" testId="surface-edge" bind={edge} options={titled(['left', 'right', 'top', 'bottom'])} />}
        {kind.value === 'float' && (
          <Options label="Position" testId="surface-position" bind={position} options={titled(['center', 'top', 'bottom', 'left', 'right'])} />
        )}
        {kind.value === 'window' && <Options label="Chrome" testId="surface-chrome" bind={chrome} options={titled(['system', 'full'])} />}
        <grid className="gap-2 grid-cols-2">
          <Field testId="surface-width" label="Width (px or %)" bind={width} placeholder="width (px or %)" />
          <Field testId="surface-height" label="Height (px or %)" bind={height} placeholder="height (px or %)" />
        </grid>
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
          <grid className="gap-2 grid-cols-3">
            <Button size="sm" testId="surface-parent-show" disabled={!!state?.visible} onClick={() => actions.showActiveSurface()}>
              Show
            </Button>
            <Button size="sm" variant="secondary" testId="surface-parent-hide" disabled={!state?.visible} onClick={() => actions.hideActiveSurface()}>
              Hide
            </Button>
            <Button size="sm" variant="destructive" testId="surface-parent-close" onClick={() => actions.closeActiveSurface()}>
              Close
            </Button>
          </grid>
        )}
        <text data-testid="surface-status" className="px-3 py-2 rounded-lg bg-surface-sunken text-sm font-mono break-all">
          {state?.message || 'No message received yet.'}
        </text>
      </Section>
    </Page>
  );
}

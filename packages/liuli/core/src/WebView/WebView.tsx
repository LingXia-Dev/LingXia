import {
  cx,
  useId,
  useLocal,
  useView,
  useWebView,
  type Child,
  type WebView as WebSession,
  type WebViewBlocked,
  type WebViewError,
  type WebViewLoad,
  type WebViewMessage,
} from '@lingxia/view';
import type { DialogDismissOn } from '../Dialog/Dialog';
import { Glyph } from '../internal/Glyph';
import { webViewRecipe as r } from './WebView.recipe';

export type WebViewExpansion =
  | {
      /** Stays inline. @default 'none' */
      expand?: 'none';
      expandLabel?: never;
      closeLabel?: never;
      isOpen?: never;
      onOpenChange?: never;
      dismissOn?: never;
      actions?: never;
    }
  | {
      /** Adds an expand button over the frame that opens the same live document in a 70% `'sheet'` or a `'full'` dialog without reloading. */
      expand: 'sheet' | 'full';
      /** The expand button's name: "Expand map". */
      expandLabel: string;
      /** The close button's name in the expanded header: "Close map". */
      closeLabel: string;
      /** Whether the expanded sheet or dialog is open; leave it out to let the expand and close buttons own it. */
      isOpen?: boolean;
      onOpenChange?: (isOpen: boolean) => void;
      /** Which requests close the expanded sheet or dialog (`'any'` includes a backdrop tap). @default 'any' */
      dismissOn?: DialogDismissOn;
      /** Footer inside the expanded sheet or dialog, such as a confirmation bar under a map. */
      actions?: Child;
    };

export type WebViewProps = WebViewExpansion & {
  /** Address or packaged path of the document. Optional when another `<webview>` already gave `web` a source. */
  src?: string;
  /** Shared `useWebView` handle; when omitted, `WebView` creates its own so expanding still keeps one live document. */
  web?: WebSession;
  /** What the embedded page shows, its accessible name and expanded heading. */
  label: string;
  /** Runs when a top-level document finishes loading. */
  onLoad?: (event: WebViewLoad) => void;
  /** Runs when the page calls `window.lingxia.postMessage`. */
  onMessage?: (message: WebViewMessage) => void;
  /** Runs when the host blocks an off-origin or disallowed navigation. */
  onBlocked?: (event: WebViewBlocked) => void;
  /** Runs when loading fails or a queued message is dropped. */
  onError?: (error: WebViewError) => void;
  /** Classes for the inline frame, such as its height or aspect ratio. */
  className?: string;
  /** On the inline `<webview>`; the expand button takes `<id>-expand` and the expanded dialog `<id>-sheet`. */
  'data-testid'?: string;
};

const closedBy = { any: 'any', closeRequest: 'closerequest', none: 'none' } as const satisfies Record<DialogDismissOn, string>;

/**
 * An embedded web document inside a framed container, with optional expansion into a 70% bottom
 * sheet or full-screen dialog that reuses the same live document without reloading.
 */
export function WebView({
  src,
  web,
  label,
  expand = 'none',
  expandLabel,
  closeLabel,
  isOpen,
  onOpenChange,
  dismissOn = 'any',
  actions,
  onLoad,
  onMessage,
  onBlocked,
  onError,
  className,
  'data-testid': testId,
}: WebViewProps) {
  const { env } = useView();
  const fallback = useWebView(useId());
  const session = web ?? fallback;
  const localOpen = useLocal('open', false);
  const controlled = isOpen !== undefined;
  const shown = expand !== 'none' && (controlled ? isOpen : localOpen.value);
  const setOpen = (next: boolean) => {
    if (!controlled) localOpen.set(next);
    onOpenChange?.(next);
  };
  const expandable = expand !== 'none';
  const panelMode = expand === 'full' ? 'full' : 'sheet';

  return (
    <view className={cx(r.root.base, className)}>
      <webview
        web={session}
        src={src}
        aria-label={label}
        onLoad={onLoad}
        onMessage={onMessage}
        onBlocked={onBlocked}
        onError={onError}
        data-testid={testId}
        className={r.webview.base}
      />
      {expandable && !shown && (
        <button
          aria-label={expandLabel}
          onClick={() => setOpen(true)}
          data-testid={testId && `${testId}-expand`}
          className={r.expand.base}
        >
          <Glyph name="expand" className={r.expandIcon.base} />
        </button>
      )}
      {shown && (
        <dialog
          open
          closedby={closedBy[dismissOn]}
          onClose={() => setOpen(false)}
          aria-label={label}
          data-testid={testId && `${testId}-sheet`}
          className={cx(r.panel.base, r.panel.mode[panelMode])}
          style={panelMode === 'full'
            ? { paddingTop: env.safeArea.top, paddingBottom: env.safeArea.bottom }
            : { paddingBottom: env.safeArea.bottom }}
        >
          <view className={r.header.base}>
            <text role="heading" headingLevel={2} className={r.title.base}>{session.title || label}</text>
            <button
              aria-label={closeLabel}
              onClick={() => setOpen(false)}
              data-testid={testId && `${testId}-close`}
              className={r.close.base}
            >
              <Glyph name="close" className={r.closeIcon.base} />
            </button>
          </view>
          <webview
            web={session}
            aria-label={label}
            data-testid={testId && `${testId}-expanded`}
            className={r.stage.base}
          />
          {actions !== undefined && <view className={r.footer.base}>{actions}</view>}
        </dialog>
      )}
    </view>
  );
}

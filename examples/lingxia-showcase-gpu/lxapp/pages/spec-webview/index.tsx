import '../../theme/index.css';
import { useLocal, useWebView, type WebViewBlocked, type WebViewError, type WebViewLoad, type WebViewMessage } from '@lingxia/view';
import { Button, Page } from '../../shared/ui';

type BootMsg = {
  kind?: string;
  epoch?: string;
  hash?: string;
  count?: number;
  seq?: number;
  error?: string;
  iframeHasLingxia?: boolean;
};

export function render() {
  const web = useWebView('spec-web');
  const src = useLocal('wv-src', 'public/embed/demo.html');
  const sheetOpen = useLocal('wv-sheet', false);
  const dialogOpen = useLocal('wv-dialog', false);
  const loadedTitle = useLocal('wv-title', '');
  const loadedUrl = useLocal('wv-url', '');
  const bootEpoch = useLocal('wv-epoch', '');
  const lastHash = useLocal('wv-hash', '');
  const htmlTaps = useLocal('wv-html-taps', 0);
  const fabTaps = useLocal('wv-fab-taps', 0);
  const origins = useLocal('wv-origin', '');
  const iframeClean = useLocal('wv-iframe-clean', 'pending');
  const coalesced = useLocal('wv-coalesced', '');
  const queued = useLocal('wv-queued', '');
  const oversizeErr = useLocal('wv-oversize', '');
  const floodLast = useLocal('wv-flood-last', 0);
  const blockedUrl = useLocal('wv-blocked', '');
  const lastErr = useLocal('wv-error', '');

  const onLoad = (ev: WebViewLoad) => {
    loadedTitle.set(ev.title);
    loadedUrl.set(ev.url);
  };

  const onMessage = (ev: WebViewMessage) => {
    origins.set(ev.origin);
    const d = (ev.data ?? {}) as BootMsg;
    if (d.kind === 'boot') {
      bootEpoch.set(d.epoch ?? '');
      lastHash.set(d.hash ?? '');
      iframeClean.set(d.iframeHasLingxia === false ? 'true' : 'false');
    } else if (d.kind === 'hash') {
      bootEpoch.set(d.epoch ?? '');
      lastHash.set(d.hash ?? '');
    } else if (d.kind === 'tap') {
      htmlTaps.set(d.count ?? 0);
      if (d.epoch) bootEpoch.set(d.epoch);
    } else if (d.kind === 'coalesce') {
      coalesced.set(coalesced.value ? `${coalesced.value},${d.seq}` : String(d.seq));
    } else if (d.kind === 'queued') {
      queued.set(queued.value ? `${queued.value},${d.seq}` : String(d.seq));
    } else if (d.kind === 'oversize') {
      oversizeErr.set(d.error ?? '');
    } else if (d.kind === 'flood') {
      floodLast.set(d.seq ?? 0);
    }
  };

  const onBlocked = (ev: WebViewBlocked) => {
    blockedUrl.set(ev.url);
  };

  const onError = (ev: WebViewError) => {
    lastErr.set(ev.code);
  };

  return (
    <Page testId="spec-webview">
      <view className="gap-3 p-3">
        <view className="relative h-48 rounded-2xl overflow-hidden border border-line">
          <webview
            web={web}
            data-testid="spec-webview-frame"
            src={src.value}
            aria-label="Embedded demo document"
            onLoad={onLoad}
            onMessage={onMessage}
            onBlocked={onBlocked}
            onError={onError}
            className="w-full h-full rounded-2xl"
          />
          <button
            data-testid="spec-webview-fab"
            aria-label="Overlay action"
            onClick={() => fabTaps.set(fabTaps.value + 1)}
            className="absolute right-3 bottom-3 px-3 py-2 rounded-xl bg-danger-solid items-center justify-center"
          >
            <text className="text-xs font-semibold text-on-danger-solid">{`FAB ${fabTaps.value}`}</text>
          </button>
        </view>

        <view className="flex-row flex-wrap gap-2">
          <Button testId="wv-btn-hash" size="sm" onClick={() => src.set('public/embed/demo.html#section-2')}>Hash</Button>
          <Button testId="wv-btn-coalesce" size="sm" onClick={() => src.set('public/embed/demo.html#coalesce')}>Coalesce</Button>
          <Button testId="wv-btn-queue" size="sm" onClick={() => src.set('public/embed/demo.html#queue')}>Queue</Button>
          <Button testId="wv-btn-oversize" size="sm" onClick={() => src.set('public/embed/demo.html#oversize')}>Oversize</Button>
          <Button testId="wv-btn-overflow" size="sm" onClick={() => src.set('public/embed/demo.html#overflow')}>Overflow</Button>
          <Button testId="wv-btn-block" size="sm" onClick={() => src.set('public/embed/demo.html#block-nav')}>Block</Button>
          <Button testId="wv-btn-bad-src" size="sm" onClick={() => src.set('tel:10086')}>BadSrc</Button>
          <Button testId="wv-btn-ungranted" size="sm" onClick={() => src.set('https://intranet/page')}>Ungranted</Button>
          <Button testId="wv-btn-reset" size="sm" onClick={() => src.set('public/embed/demo.html')}>Reset</Button>
          <Button testId="wv-btn-sheet" size="sm" onClick={() => sheetOpen.set(true)}>Sheet</Button>
          <Button testId="wv-btn-dialog" size="sm" onClick={() => dialogOpen.set(true)}>Dialog</Button>
        </view>

        <text data-testid="wv-state-load">{`title=${loadedTitle.value}|origin=${origins.value}|iframeClean=${iframeClean.value}`}</text>
        <text data-testid="wv-state-epoch">{`epoch=${bootEpoch.value}|hash=${lastHash.value}|sheet=${sheetOpen.value}`}</text>
        <text data-testid="wv-state-taps">{`html=${htmlTaps.value}|fab=${fabTaps.value}`}</text>
        <text data-testid="wv-state-msg">{`coalesced=${coalesced.value}|queued=${queued.value}|oversize=${oversizeErr.value}|flood=${floodLast.value}`}</text>
        <text data-testid="wv-state-err">{`blocked=${blockedUrl.value}|err=${lastErr.value}`}</text>

        <dialog
          open={sheetOpen.value}
          closedby="any"
          onClose={() => sheetOpen.set(false)}
          aria-label="Expanded embedded sheet"
          data-testid="wv-sheet"
          className="mt-auto mb-0 mx-0 w-full h-[70%] rounded-t-2xl bg-surface overflow-hidden"
        >
          <view className="flex-row items-center justify-between px-4 py-2 border-b border-line">
            <text className="text-sm font-semibold">{web.title || 'Embed Demo'}</text>
            <Button testId="wv-sheet-close" size="sm" onClick={() => sheetOpen.set(false)}>Close sheet</Button>
          </view>
          <webview
            web={web}
            data-testid="spec-webview-sheet-frame"
            aria-label="Embedded demo document"
            className="w-full grow"
          />
        </dialog>

        <dialog
          open={dialogOpen.value}
          onClose={() => dialogOpen.set(false)}
          aria-label="Modal over WebView"
          data-testid="wv-modal"
          className="p-4 rounded-2xl bg-surface gap-3 w-64"
        >
          <text className="text-sm font-semibold">Modal covering WebView</text>
          <Button testId="wv-modal-close" size="sm" onClick={() => dialogOpen.set(false)}>Close modal</Button>
        </dialog>
      </view>
    </Page>
  );
}

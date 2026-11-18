import { useLocal, useWebView } from '@lingxia/view';
import { Button } from '../Button/Button';
import { WebView, type WebViewLabels } from './WebView';

const labels: WebViewLabels = {
  expand: 'Expand map',
  close: 'Close map',
};

/** An inline card with a corner expand button that opens the same document in a 70% bottom sheet. */
export function ExpandableSheet() {
  const hash = useLocal('section', '#overview');
  return (
    <view className="gap-2">
      <WebView
        src={`https://example.com/map${hash.value}`}
        label="Delivery area map"
        labels={labels}
        expand="sheet"
        data-testid="sheet-web"
      />
      <Button
        label={hash.value === '#overview' ? 'Center on pin' : 'Show overview'}
        size="sm"
        width="content"
        onClick={() => hash.set(hash.value === '#overview' ? '#pin' : '#overview')}
        data-testid="toggle-hash"
      />
    </view>
  );
}

/** Controlled full-screen expansion with a native confirmation footer below the web document. */
export function FullScreenWithActions() {
  const web = useWebView('liuli-web-picker');
  const open = useLocal('picker-open', false);
  return (
    <WebView
      web={web}
      src="https://example.com/picker"
      label="Pick a meeting point"
      labels={labels}
      expand="full"
      isExpanded={open.value}
      onExpandedChange={next => open.set(next)}
      actions={<Button label="Confirm location" variant="primary" onClick={() => open.set(false)} data-testid="confirm" />}
      data-testid="full-web"
    />
  );
}

/** The home tile. */
export function Specimen() {
  return <WebView src="https://example.com/embed" label="Embedded page" className="w-28 h-16" />;
}

import { useLocal, useWebView } from '@lingxia/view';
import { Button } from '../Button/Button';
import { WebView } from './WebView';

// A page bundled with the gallery, so the examples work offline; an app passes its own path or address.
const map = 'public/web/map.html';

/** An inline card with a corner expand button that opens the same document in a 70% bottom sheet. */
export function ExpandableSheet() {
  const hash = useLocal('section', '#overview');
  return (
    <view className="gap-2">
      <WebView
        src={`${map}${hash.value}`}
        label="Delivery area map"
        expand="sheet"
        expandLabel="Expand map"
        closeLabel="Close map"
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
      src={`${map}#picker`}
      label="Pick a meeting point"
      expand="full"
      expandLabel="Expand the map"
      closeLabel="Close the map"
      isOpen={open.value}
      onOpenChange={next => open.set(next)}
      actions={<Button label="Confirm location" variant="primary" onClick={() => open.set(false)} data-testid="confirm" />}
      data-testid="full-web"
    />
  );
}

/** The home tile. */
export function Specimen() {
  return <WebView src={`${map}#overview`} label="Delivery area map" className="w-28 h-16" />;
}

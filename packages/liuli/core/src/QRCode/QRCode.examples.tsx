import { QRCode } from './QRCode';

/** A guest network to join: the code, and the same details in words for whoever cannot scan. */
export function Join() {
  return (
    <view className="flex-row items-center gap-4">
      <QRCode value="WIFI:T:WPA;S:Harbour Café Guest;P:tide-pool-73;;" label="Code to join Harbour Café Guest" data-testid="join" />
      <view className="shrink min-w-0 gap-1">
        <text className="text-callout font-semibold text-text">Harbour Café Guest</text>
        <text className="text-footnote text-muted">Password tide-pool-73</text>
      </view>
    </view>
  );
}

/** A link to open on a phone, with the address under it. */
export function Link() {
  return (
    <view className="items-start gap-2">
      <QRCode value="https://example.com/invite/7Q4M" label="Code for the invite link" size="sm" data-testid="invite" />
      <text className="text-footnote text-muted">example.com/invite/7Q4M</text>
    </view>
  );
}

/** The three sizes. */
export function Sizes() {
  return (
    <view className="flex-row flex-wrap items-end gap-4">
      <QRCode value="https://example.com" label="Small code" size="sm" />
      <QRCode value="https://example.com" label="Medium code" />
      <QRCode value="https://example.com" label="Large code" size="lg" />
    </view>
  );
}

/** The home tile. */
export function Specimen() {
  return <QRCode value="https://example.com/liuli" label="Code for Liuli" size="sm" />;
}

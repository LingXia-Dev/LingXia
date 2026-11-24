import { hueFor } from '@liulidesign/core/hue';

const HUE_FILL = ['', 'bg-hue-1-soft', 'bg-hue-2-soft', 'bg-hue-3-soft', 'bg-hue-4-soft', 'bg-hue-5-soft', 'bg-hue-6-soft', 'bg-hue-7-soft', 'bg-hue-8-soft'];

/** A site's monogram on its hue, drawn as an Avatar's rounded tile; no favicon route reaches Logic yet. */
export function SiteIcon({ host, size = 'md' }: { host: string; size?: 'sm' | 'md' }) {
  // The site's own name, not a subdomain: cn.bing.com shows B.
  const name = host.split('.').slice(-2)[0] ?? host;
  const letter = (name.match(/[\p{L}\p{N}]/u)?.[0] ?? '?').toUpperCase();
  return (
    <view aria-hidden className={`shrink-0 items-center justify-center ${size === 'sm' ? 'size-6 rounded-checkbox' : 'size-8 rounded-segment'} ${HUE_FILL[hueFor(name)]}`}>
      <text className={`${size === 'sm' ? 'text-caption' : 'text-footnote'} font-semibold text-text`}>{letter}</text>
    </view>
  );
}

import { useLocal, type Local } from '@lingxia/view';
import { liuliText } from '../i18n/text';
import { Glyph } from '../internal/Glyph';
import { PageControl } from '../PageControl/PageControl';
import { lightboxRecipe as r } from './Lightbox.recipe';
import { dataOf } from '../internal/data';

export interface LightboxImage {
  src: string;
  /** What the picture shows. */
  alt: string;
  /** A line under the picture, in view while it is. */
  caption?: string;
}

/** Which picture is in view: a View-local cell, or a value with its change handler, or neither (the lightbox holds it). */
export type LightboxIndex =
  | {
      /** A View-local cell holding which picture is in view, from 0. Set it before opening to start there. */
      bind?: Local<number>;
      index?: never;
      onIndexChange?: never;
    }
  | {
      bind?: never;
      /** The picture in view, from 0, held by the caller: in Logic state, or to open at the one pressed. */
      index: number;
      /** Runs with the picture a swipe or a dot moves to. */
      onIndexChange: (index: number) => void;
    };

export type LightboxProps = LightboxIndex & {
  /** Whether it is shown. A closed lightbox renders nothing. */
  isOpen: boolean;
  /** Called with `false` on a close request: Escape, back, the close button. */
  onOpenChange: (isOpen: boolean) => void;
  /** The lightbox's accessible name: "Photos of the house". */
  label: string;
  images: readonly LightboxImage[];
  /** The picture it opens on when the lightbox holds it. @default 0 */
  defaultIndex?: number;
  /** The close button's name: "Close". Defaults to "Close" in the page's language. */
  closeLabel?: string;
  /** Where the viewer is, written by the app: `(page, count) => \`${page + 1} of ${count}\``. Shown at the top, and each dot's name. Defaults to "2 of 5" in the page's language. */
  pageLabel?: (page: number, count: number) => string;
  'data-testid'?: string;
};

/** Pictures one at a time over the whole screen, shown whole. */
export function Lightbox({ isOpen, onOpenChange, label, images, bind, index, onIndexChange, defaultIndex = 0, closeLabel = liuliText('@liuli.close'), pageLabel = (page: number, count: number) => liuliText('@liuli.pictureOf', { page: page + 1, count }), 'data-testid': testId, ...attributes }: LightboxProps) {
  const local = useLocal('page', defaultIndex);
  const cell = bind ?? local;
  const page = onIndexChange !== undefined ? { value: index ?? 0 } : cell;
  const follow = onIndexChange !== undefined ? { snapIndex: index ?? 0, onSnapChange: onIndexChange } : { bind: cell };
  const dotsFollow = onIndexChange !== undefined ? { value: index ?? 0, onChange: onIndexChange } : { bind: cell };
  const at = Math.min(Math.max(0, Math.round(page.value)), Math.max(0, images.length - 1));
  const close = () => onOpenChange(false);
  return (
    <dialog
      open={isOpen}
      onClose={close}
      closedby="closerequest"
      aria-label={label}
      data-testid={testId} {...dataOf(attributes)}
      className={r.panel.base}
    >
      <view className={r.bar.base}>
        <text role="status" className={r.count.base}>{images.length > 1 ? pageLabel(at, images.length) : ''}</text>
        <button aria-label={closeLabel} onClick={close} data-testid={testId && `${testId}-close`} className={r.close.base}>
          <Glyph name="close" className={r.closeIcon.base} />
        </button>
      </view>
      <scroll {...follow} className={r.scroll.base}>
        <view role="list" aria-label={label} className={r.track.base}>
          {images.map((image, index) => (
            <view key={`image-${index}`} role="listitem" className={r.page.base}>
              <image src={image.src} alt={image.alt} className={r.image.base} />
            </view>
          ))}
        </view>
      </scroll>
      {images[at]?.caption !== undefined && <text className={r.caption.base}>{images[at].caption}</text>}
      {images.length > 1 && (
        <PageControl label={label} count={images.length} {...dotsFollow} tone="on-media" pageLabel={index => pageLabel(index, images.length)} data-testid={testId && `${testId}-dots`} />
      )}
    </dialog>
  );
}

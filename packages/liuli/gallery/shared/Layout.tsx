import { cardRecipe } from '@liulidesign/core/Card/recipe';
import { cx, useView, type Child } from '@lingxia/view';

// The gallery's one page skeleton: every page takes its rhythm from these, never its own classes.

/** A gallery page: a scrolling column with the page gutter and section spacing. */
export function GalleryPage({ children }: { children: Child }) {
  const { env } = useView();
  return (
    <page className="bg-canvas">
      <scroll className="grow">
        <view className="px-4 pt-4 pb-10 gap-8">{children}</view>
      </scroll>
      {/* Test probe: specs read the bottom inset from its height (inset + 1); no handlers, so taps pass through. */}
      <view aria-hidden data-testid="safe-area-bottom" className="absolute start-0 bottom-0 w-px" style={{ height: env.insets.bottom + 1 }} />
    </page>
  );
}

/** One section of a page: a small heading, its content 12 below, blocks 24 apart. */
export function PageSection({ title, children }: { title: string; children: Child }) {
  return (
    <view className="gap-3">
      <text role="heading" headingLevel={2} className="text-footnote font-medium text-muted">{title}</text>
      <view className="gap-6">{children}</view>
    </view>
  );
}

/**
 * The quiet surface a specimen sits on, one step off the page, with an optional caption below.
 * `bare` drops the surface for containers (cards, lists) that would otherwise sit in a second frame.
 */
export function Stage({ caption, bare = false, children }: { caption?: string; bare?: boolean; children: Child }) {
  return (
    <view className="gap-2">
      <view className={bare ? 'grow gap-3' : 'grow gap-3 overflow-hidden rounded-container border border-text/12 bg-surface p-5'}>{children}</view>
      {caption !== undefined && <text className="text-footnote text-muted">{caption}</text>}
    </view>
  );
}

/** Stages side by side, two to a row; used for a component's states. */
export function StageGrid({ children, stack = false }: { children: Child; stack?: boolean }) {
  return <view className={stack ? 'grid grid-cols-1 md:grid-cols-2 gap-x-3 gap-y-6' : 'grid grid-cols-2 gap-x-3 gap-y-6'}>{children}</view>;
}

/** A component page's opening: when to use it, then the hero specimen. */
export function Lead({ text, bare = false, children }: { text: string; bare?: boolean; children: Child }) {
  return (
    <view className="gap-4">
      <text className="text-body text-muted">{text}</text>
      <Stage bare={bare}>{children}</Stage>
    </view>
  );
}

/** The home page's opening block. */
export function Masthead({ version, status }: { version: string; status: string }) {
  return (
    <view className="gap-1">
      <view className="flex-row items-baseline gap-2">
        <text className="text-display font-semibold tracking-tight text-text">Liuli</text>
        <text className="text-title text-muted">琉璃</text>
        <text className="ms-auto text-footnote text-faint">{`${version} · ${status}`}</text>
      </view>
      <text className="text-body text-muted">LingXia's design system for GPU pages.</text>
    </view>
  );
}

/** A category page's members: a heading and a grid of tiles. */
export function Category({ title, children }: { title: string; children: Child }) {
  return (
    <view className="gap-3">
      <text role="heading" headingLevel={2} className="text-footnote font-medium text-muted">{title}</text>
      <view className="grid grid-cols-1 min-[360px]:grid-cols-2 md:grid-cols-3 lg:grid-cols-4 gap-3">{children}</view>
    </view>
  );
}

export interface TileProps {
  name: string;
  /** One line, never cut. */
  tagline: string;
  /** The last tile of an odd row spans both columns of the phone grid. */
  isWide?: boolean;
  onOpen: () => void;
  testId: string;
  /** The live specimen, drawn on the tile's stage and hidden from assistive tech. */
  children: Child;
}

/**
 * A tile: a specimen on a stage, the name and one line. The press target is a transparent
 * button laid over the whole tile, since a specimen may hold buttons of its own.
 */
export function Tile({ name, tagline, isWide = false, onOpen, testId, children }: TileProps) {
  return (
    // A tile is a card; taking the card's own frame also brings Liuli's tokens to every page laid out with this file.
    <view className={cx('relative overflow-hidden', cardRecipe.root.base, cardRecipe.root.variant.outlined, isWide && 'min-[360px]:col-span-2 md:col-span-1')}>
      <view aria-hidden className="h-28 items-center-safe justify-center-safe overflow-hidden p-3 bg-surface-sunken border-b border-text/8">
        {children}
      </view>
      <view className="px-3 pt-2.5 pb-3 gap-0.5">
        <text className="text-body font-medium text-text">{name}</text>
        <text className="text-footnote text-muted">{tagline}</text>
      </view>
      <button
        aria-label={name}
        onClick={onOpen}
        data-testid={testId}
        className="absolute inset-0 rounded-container transition-colors duration-150 ease-out hover:bg-text/3 active:bg-text/6"
      />
    </view>
  );
}

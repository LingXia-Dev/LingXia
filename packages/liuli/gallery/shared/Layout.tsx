import { cardRecipe } from '@liulidesign/core/Card/recipe';
import { createContext, cx, useBox, useContext, useLocal, useView, type Child } from '@lingxia/view';

// The gallery's one page skeleton: every page takes its rhythm from these, never its own classes.

/** A gallery page: a scrolling column with the page gutter and section spacing. */
export function GalleryPage({ children, isFilled = false }: { children: Child; isFilled?: boolean }) {
  const { env } = useView();
  return (
    <page className="bg-canvas">
      {isFilled ? (
        // A pattern that owns its height: it fills what the page leaves, so the keyboard shortens it and nothing scrolls under.
        <view className="grow basis-0 min-h-0 px-4 pt-4 pb-3">{children}</view>
      ) : (
        <scroll className="grow">
          <view className="px-4 pt-4 pb-10 gap-8">
            {/* Test probe: the scroll's start, so a capture can bring the page back to its top. */}
            <view aria-hidden data-testid="page-start" className="absolute start-0 top-0 size-px" />
            {children}
          </view>
        </scroll>
      )}
      {/* Test probe: specs read the bottom inset from its height (inset + 1); no handlers, so taps pass through. */}
      <view aria-hidden data-testid="safe-area-bottom" className="absolute start-0 bottom-0 w-px" style={{ height: env.insets.bottom + 1 }} />
      {/* Test probe: which form the page has taken, so a capture can refuse the wrong one. */}
      <view aria-hidden data-testid={env.sizeClass === 'regular' ? 'form-regular' : 'form-compact'} className="absolute start-0 top-0 size-px" />
      {/* Test probe: the appearance the page is drawn in, so a capture waits until a pinned one has taken. */}
      <view aria-hidden data-testid={`page-appearance-${env.appearance}`} className="absolute start-0 top-0 size-px" />
    </page>
  );
}

/** One section of a page: a small heading, an optional sentence about it, its content 12 below, blocks 24 apart. */
export function PageSection({ title, note, children }: { title: string; note?: string; children: Child }) {
  return (
    <view className="gap-3">
      <view className="gap-1">
        <text role="heading" headingLevel={2} className="text-footnote font-medium text-muted">{title}</text>
        {note !== undefined && <text className="text-footnote text-muted">{note}</text>}
      </view>
      <view className="gap-6">{children}</view>
    </view>
  );
}

/** In a `StageGrid`: tells the grid that a stage's content does not fit its stage. */
const StageFit = createContext<((isOver: boolean) => void) | null>(null);

/**
 * The quiet surface a specimen sits on, one step off the page, under a caption that says what it shows.
 * The caption comes first, so a specimen is read as an example before it is seen.
 * `bare` drops the surface for containers (cards, lists) that would otherwise sit in a second frame.
 */
export function Stage({ caption, bare = false, children }: { caption?: string; bare?: boolean; children: Child }) {
  const report = useContext(StageFit);
  // In a grid the specimen is laid no narrower than its widest line, so a line that would wrap shows as overflow.
  const room = useLocal('room', 0);
  const need = useLocal('need', 0);
  const body = report === null ? children : (
    <view onLayout={box => { need.set(box.width); if (room.value > 0) report(box.width > room.value + 0.5); }} className="min-w-max gap-3">{children}</view>
  );
  return (
    <view className="gap-2">
      {caption !== undefined && <text className="text-footnote text-text">{caption}</text>}
      <view
        onLayout={report === null ? undefined : box => { room.set(box.width); if (need.value > 0) report(need.value > box.width + 0.5); }}
        className={bare ? 'grow gap-3' : 'grow gap-3 overflow-hidden rounded-container border border-line bg-surface p-5'}
      >
        {body}
      </view>
    </view>
  );
}

/**
 * Stages side by side, two to a row; used for a component's states. A grid whose stages cannot each
 * hold their widest line (a label, placeholder, value or message) beside another holds one to a row.
 */
export function StageGrid({ children, stack = false }: { children: Child; stack?: boolean }) {
  const grid = useBox('stage-grid');
  const em = useBox('stage-em');
  // The grid's width and the body type's size: a change of either lays the grid two-up again and measures.
  // The stages remount on a change, so each report carries the key it was measured under; the text size
  // reaches script only through the probe's box, after the stages' own layout.
  const key = `${grid.value?.width ?? 0}:${em.value?.width ?? 0}`;
  const oneUpAt = useLocal<string | null>('one-up-at', null);
  const isOneUp = oneUpAt.value === key;
  const report = (isOver: boolean) => { if (isOver && !isOneUp && grid.value && em.value) oneUpAt.set(key); };
  return (
    <view>
      <view aria-hidden className="h-0 overflow-hidden">
        <text box={em} className="self-start text-body">M</text>
      </view>
      {/* One to a row, the stages stop measuring, so a specimen wraps to its stage instead of running past the screen. */}
      <StageFit.Provider value={isOneUp ? null : report}>
        <view key={key} box={grid} className={isOneUp ? 'grid grid-cols-1 gap-y-6' : stack ? 'grid grid-cols-1 md:grid-cols-2 gap-x-3 gap-y-6' : 'grid grid-cols-2 gap-x-3 gap-y-6'}>{children}</view>
      </StageFit.Provider>
    </view>
  );
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
  const { env } = useView();
  // The title row shares the top with the host's capsule: a long title stops 8 short of it, on the
  // side it floats (less the page's 16 gutter). Nothing else sits in that band.
  const capsule = env.capsule;
  const clear = capsule === null
    ? undefined
    : capsule.x > env.viewport.width / 2
      ? { paddingRight: Math.max(0, env.viewport.width - capsule.x - 16 + 8) }
      : { paddingLeft: Math.max(0, capsule.x + capsule.width - 16 + 8) };
  return (
    <view className="gap-1">
      <view className="flex-row items-baseline gap-2" style={clear}>
        <text className="text-display font-semibold tracking-tight text-text">Liuli</text>
        <text className="text-title text-muted">琉璃</text>
      </view>
      <view className="flex-row flex-wrap items-baseline gap-x-2">
        <text className="text-body text-muted">LingXia's design system for GPU pages.</text>
        <text className="text-footnote text-faint" data-testid="masthead-version">{`${version} · ${status}`}</text>
      </view>
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
      <view aria-hidden className="h-28 items-center-safe justify-center-safe overflow-hidden p-3 bg-surface-sunken border-b border-line">
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

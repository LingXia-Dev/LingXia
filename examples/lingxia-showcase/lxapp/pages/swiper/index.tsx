import React from 'react';
import { LxMediaSwiper, useLxPage } from '@lingxia/react';
import '../../tailwind.css';

type MediaSwiperElement = HTMLElement & {
  next(): void;
  previous(): void;
  goToIndex(index: number): void;
};

type SwiperItem = {
  id: string;
  type: 'image' | 'video';
  src: string;
};

type ObjectFit = 'cover' | 'contain' | 'fill';
type Animation = 'slide' | 'none';
type Direction = 'horizontal' | 'vertical';

type PageData = {
  items?: SwiperItem[];
  index?: number;
  autoplay?: boolean;
  loop?: boolean;
  dots?: boolean;
  objectFit?: ObjectFit;
  animation?: Animation;
  direction?: Direction;
  peek?: number;
  eventLog?: string;
  busy?: boolean;
};

type PageActions = {
  data?: PageData;
  pickImages?: () => void;
  pickVideos?: () => void;
  removeCurrent?: () => void;
  clearAll?: () => void;
  toggleAutoplay?: () => void;
  toggleLoop?: () => void;
  toggleDots?: () => void;
  setObjectFit?: (params: { fit: ObjectFit }) => void;
  setAnimation?: (params: { animation: Animation }) => void;
  setDirection?: (params: { direction: Direction }) => void;
  setPeek?: (params: { value: number }) => void;
  onSwiperChange?: (event: Event) => void;
  onSwiperTransitionEnd?: (event: Event) => void;
  onSwiperEndReached?: (event: Event) => void;
  onSwiperTap?: (event: Event) => void;
  onSwiperVideoEnded?: (event: Event) => void;
  onSwiperError?: (event: Event) => void;
};

const fits: ObjectFit[] = ['cover', 'contain', 'fill'];
const animations: Animation[] = ['slide', 'none'];
const directions: Direction[] = ['horizontal', 'vertical'];
const peekPresets: number[] = [0, 16, 32, 48];

export default function SwiperPage() {
  const { data, actions } = useLxPage<PageData, PageActions>();
  const swiperRef = React.useRef<MediaSwiperElement | null>(null);

  const items = data?.items ?? [];
  const index = typeof data?.index === 'number' ? data.index : 0;
  const autoplay = !!data?.autoplay;
  const loop = !!data?.loop;
  const dots = data?.dots ?? true;
  const objectFit: ObjectFit = data?.objectFit ?? 'cover';
  const animation: Animation = data?.animation ?? 'slide';
  const direction: Direction = data?.direction ?? 'horizontal';
  const peek = data?.peek ?? 0;
  const eventLog = data?.eventLog ?? 'Pick media to start';
  const busy = !!data?.busy;

  const {
    pickImages,
    pickVideos,
    removeCurrent,
    clearAll,
    toggleAutoplay,
    toggleLoop,
    toggleDots,
    setObjectFit,
    setAnimation,
    setDirection,
    setPeek,
    onSwiperChange,
    onSwiperTransitionEnd,
    onSwiperEndReached,
    onSwiperTap,
    onSwiperVideoEnded,
    onSwiperError,
  } = actions;

  return (
    <div className="bg-gray-100 min-h-screen">
      <div className="px-4 py-4 space-y-3 pb-8">
        {/* Header */}
        <div className="flex items-center justify-between">
          <div className="flex items-center gap-2">
            <div className="w-8 h-8 bg-gradient-to-br from-orange-500 to-rose-500 rounded-lg flex items-center justify-center">
              <svg viewBox="0 0 24 24" fill="none" stroke="white" strokeWidth="2" className="w-4 h-4">
                <rect x="3" y="5" width="18" height="14" rx="2" />
                <circle cx="9" cy="11" r="1.5" fill="white" />
                <path d="M21 15l-5-5-9 9" />
              </svg>
            </div>
            <div>
              <div className="text-base font-semibold text-gray-900">Media Swiper</div>
            </div>
          </div>
          <div className="bg-gray-900 text-emerald-400 font-mono text-xs px-3 py-1.5 rounded-lg w-[180px] truncate">
            {eventLog}
          </div>
        </div>

        {/* Stage */}
        <div className="bg-black rounded-xl overflow-hidden">
          {items.length === 0 ? (
            <div
              className="flex items-center justify-center text-white/60 text-sm"
              style={{ aspectRatio: '16 / 10' }}
            >
              Tap "Add Image" or "Add Video" below
            </div>
          ) : (
            <LxMediaSwiper
              ref={(el) => {
                swiperRef.current = el as MediaSwiperElement | null;
              }}
              id="lx-media-swiper-demo"
              items={items}
              index={index}
              autoplay={autoplay}
              loop={loop}
              dots={dots ? { color: '#ffffff66', activeColor: '#ffffff' } : false}
              objectFit={objectFit}
              animation={animation}
              direction={direction}
              peek={peek > 0 ? peek : undefined}
              controls={false}
              muted
              style={{ display: 'block', width: '100%', aspectRatio: '16 / 10', borderRadius: 12 }}
              onChange={onSwiperChange}
              onTransitionEnd={onSwiperTransitionEnd}
              onEndReached={onSwiperEndReached}
              onTap={onSwiperTap}
              onVideoEnded={onSwiperVideoEnded}
              onError={onSwiperError}
            />
          )}
        </div>

        {/* Source */}
        <div className="bg-white rounded-xl shadow-sm border border-gray-200 p-4 space-y-3">
          <div className="text-xs text-gray-400 uppercase tracking-wider font-semibold">Add Media</div>
          <div className="grid grid-cols-2 gap-2">
            <button
              onClick={() => pickImages?.()}
              disabled={busy}
              className="px-3 py-2.5 rounded-lg bg-gradient-to-r from-blue-500 to-cyan-500 text-white text-sm font-medium active:scale-95 disabled:opacity-50 transition-transform"
            >
              Add Image
            </button>
            <button
              onClick={() => pickVideos?.()}
              disabled={busy}
              className="px-3 py-2.5 rounded-lg bg-gradient-to-r from-purple-500 to-pink-500 text-white text-sm font-medium active:scale-95 disabled:opacity-50 transition-transform"
            >
              Add Video
            </button>
          </div>
        </div>

        {/* Controls */}
        <div className="bg-white rounded-xl shadow-sm border border-gray-200 p-4 space-y-3">
          <div className="text-xs text-gray-400 uppercase tracking-wider font-semibold">Navigation</div>
          <div className="grid grid-cols-4 gap-2">
            <button
              onClick={() => swiperRef.current?.previous()}
              disabled={items.length < 2}
              className="px-3 py-2 rounded-lg bg-gray-100 text-gray-700 text-sm font-medium active:scale-95 disabled:opacity-40"
            >
              Prev
            </button>
            <button
              onClick={() => swiperRef.current?.next()}
              disabled={items.length < 2}
              className="px-3 py-2 rounded-lg bg-gray-900 text-white text-sm font-medium active:scale-95 disabled:opacity-40"
            >
              Next
            </button>
            <button
              onClick={() => removeCurrent?.()}
              disabled={items.length === 0}
              className="px-3 py-2 rounded-lg bg-amber-50 text-amber-700 text-sm font-medium active:scale-95 disabled:opacity-40"
            >
              Remove
            </button>
            <button
              onClick={() => clearAll?.()}
              disabled={items.length === 0}
              className="px-3 py-2 rounded-lg bg-rose-50 text-rose-700 text-sm font-medium active:scale-95 disabled:opacity-40"
            >
              Clear
            </button>
          </div>
        </div>

        {/* Options */}
        <div className="bg-white rounded-xl shadow-sm border border-gray-200 p-4 space-y-3">
          <div className="text-xs text-gray-400 uppercase tracking-wider font-semibold">Options</div>

          <div className="flex items-center justify-between">
            <div>
              <div className="text-sm font-medium text-gray-900">Autoplay</div>
              <div className="text-xs text-gray-500">Cycle every 5s</div>
            </div>
            <Toggle on={autoplay} onClick={() => toggleAutoplay?.()} />
          </div>

          <div className="flex items-center justify-between">
            <div>
              <div className="text-sm font-medium text-gray-900">Loop</div>
              <div className="text-xs text-gray-500">Wrap past last item</div>
            </div>
            <Toggle on={loop} onClick={() => toggleLoop?.()} />
          </div>

          <div className="flex items-center justify-between">
            <div>
              <div className="text-sm font-medium text-gray-900">Dots</div>
              <div className="text-xs text-gray-500">Native page indicator</div>
            </div>
            <Toggle on={dots} onClick={() => toggleDots?.()} />
          </div>

          <div className="space-y-2">
            <div className="text-sm font-medium text-gray-900">Object fit</div>
            <div className="grid grid-cols-3 gap-1 bg-gray-100 rounded-lg p-1">
              {fits.map((fit) => (
                <button
                  key={fit}
                  onClick={() => setObjectFit?.({ fit })}
                  className={`py-1.5 px-2 rounded-md text-xs font-medium transition-colors ${
                    objectFit === fit ? 'bg-white text-gray-900 shadow-sm' : 'text-gray-500'
                  }`}
                >
                  {fit}
                </button>
              ))}
            </div>
          </div>

          <div className="space-y-2">
            <div className="text-sm font-medium text-gray-900">Animation</div>
            <div className="text-xs text-gray-500">v1 supports slide and none only</div>
            <div className="grid grid-cols-2 gap-1 bg-gray-100 rounded-lg p-1">
              {animations.map((a) => (
                <button
                  key={a}
                  onClick={() => setAnimation?.({ animation: a })}
                  className={`py-1.5 px-2 rounded-md text-xs font-medium transition-colors ${
                    animation === a ? 'bg-white text-gray-900 shadow-sm' : 'text-gray-500'
                  }`}
                >
                  {a}
                </button>
              ))}
            </div>
          </div>

          <div className="space-y-2">
            <div className="text-sm font-medium text-gray-900">Direction</div>
            <div className="grid grid-cols-2 gap-1 bg-gray-100 rounded-lg p-1">
              {directions.map((d) => (
                <button
                  key={d}
                  onClick={() => setDirection?.({ direction: d })}
                  className={`py-1.5 px-2 rounded-md text-xs font-medium transition-colors ${
                    direction === d ? 'bg-white text-gray-900 shadow-sm' : 'text-gray-500'
                  }`}
                >
                  {d}
                </button>
              ))}
            </div>
          </div>

          <div className="space-y-2">
            <div className="text-sm font-medium text-gray-900">Peek</div>
            <div className="text-xs text-gray-500">Adjacent page hint (px each side)</div>
            <div className="grid grid-cols-4 gap-1 bg-gray-100 rounded-lg p-1">
              {peekPresets.map((value) => (
                <button
                  key={value}
                  onClick={() => setPeek?.({ value })}
                  className={`py-1.5 px-2 rounded-md text-xs font-medium transition-colors ${
                    peek === value ? 'bg-white text-gray-900 shadow-sm' : 'text-gray-500'
                  }`}
                >
                  {value === 0 ? 'off' : `${value}px`}
                </button>
              ))}
            </div>
          </div>
        </div>

        {/* Status */}
        <div className="bg-white rounded-xl shadow-sm border border-gray-200 px-4 py-3 flex items-center justify-between text-xs">
          <div className="text-gray-500">
            {items.length > 0 ? `Item ${index + 1} of ${items.length}` : 'No items'}
          </div>
          <div className="font-mono text-gray-400">
            {items.length > 0 ? items[index]?.type : '—'}
          </div>
        </div>
      </div>
    </div>
  );
}

function Toggle({ on, onClick }: { on: boolean; onClick: () => void }) {
  return (
    <button
      onClick={onClick}
      className={`w-11 h-6 rounded-full relative transition-colors ${
        on ? 'bg-emerald-500' : 'bg-gray-300'
      }`}
    >
      <span
        className={`absolute top-0.5 left-0.5 w-5 h-5 rounded-full bg-white shadow-sm transition-transform ${
          on ? 'translate-x-5' : 'translate-x-0'
        }`}
      />
    </button>
  );
}

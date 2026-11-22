import { definePage, lx } from '@lingxia/logic';
import { errorMessage } from "../../shared/lib/errors";

interface SwiperItem {
  id: string;
  type: 'image' | 'video';
  src: string;
}

export type ObjectFit = 'cover' | 'contain' | 'fill';
export type Animation = 'slide' | 'none';
export type Direction = 'horizontal' | 'vertical';

interface SwiperData {
  items: SwiperItem[];
  index: number;
  autoplay: boolean;
  loop: boolean;
  dots: boolean;
  objectFit: ObjectFit;
  animation: Animation;
  direction: Direction;
  peek: number;
  eventLog: string;
  busy: boolean;
}

let nextItemId = 1;
function makeId() {
  const id = `picked-${nextItemId}`;
  nextItemId += 1;
  return id;
}

function clampIndex(value: number, count: number) {
  if (count <= 0) return 0;
  return Math.max(0, Math.min(count - 1, Math.trunc(value)));
}

const FITS: readonly ObjectFit[] = ["cover", "contain", "fill"];
const ANIMATIONS: readonly Animation[] = ["slide", "none"];
const DIRECTIONS: readonly Direction[] = ["horizontal", "vertical"];

const initialData: SwiperData = {
  items: [],
  index: 0,
  autoplay: false,
  loop: false,
  dots: true,
  objectFit: "cover",
  animation: "slide",
  direction: "horizontal",
  peek: 0,
  eventLog: "Pick media to start",
  busy: false,
};

export default definePage({
  data: initialData,
  setup(page) {
    const helpers = {
      async addMedia(mediaType: ("image" | "video")[], count: number) {
        if (page.data.busy) return;
        page.update(d => { d.busy = true; });
        try {
          const result = await lx.pick.media({
            mediaType,
            sourceType: ["album", "camera"],
            limit: count,
            camera: "back",
            maxDurationMs: 60_000,
          });
          if (result.status === 'canceled') {
            page.update(d => { d.busy = false; });
            return;
          }
          const picked = result.files.map((file): SwiperItem => ({
            id: makeId(),
            type: file.type.startsWith("video/") ? "video" : "image",
            src: file.path,
          }));
          page.update(d => {
            d.items.push(...picked);
            d.eventLog = `added ${picked.length} ${picked.length === 1 ? "item" : "items"}`;
            d.busy = false;
          });
        } catch (error) {
          page.update(d => { d.busy = false; });
          lx.ui.toast(errorMessage(error, "chooseMedia failed"));
        }
      },
    };
    const actions = {
      async pickImages() {
        await helpers.addMedia(["image"], 9);
      },
      async pickVideos() {
        await helpers.addMedia(["video"], 3);
      },
      removeCurrent() {
        if (page.data.items.length === 0) return;
        page.update(d => {
          d.items.splice(d.index, 1);
          d.index = clampIndex(d.index, d.items.length);
          d.eventLog = d.items.length > 0 ? "removed" : "cleared";
        });
      },
      clearAll() {
        page.update(d => {
          d.items = [];
          d.index = 0;
          d.eventLog = "cleared";
        });
      },
      toggleAutoplay() {
        page.update(d => {
          d.autoplay = !d.autoplay;
          d.eventLog = d.autoplay ? "autoplay on" : "autoplay off";
        });
      },
      toggleLoop() {
        page.update(d => {
          d.loop = !d.loop;
          d.eventLog = d.loop ? "loop on" : "loop off";
        });
      },
      toggleDots() {
        page.update(d => { d.dots = !d.dots; });
      },
      setObjectFit({ fit }: { fit: ObjectFit }) {
        if (FITS.includes(fit)) page.update(d => { d.objectFit = fit; });
      },
      setAnimation({ animation }: { animation: Animation }) {
        if (ANIMATIONS.includes(animation)) page.update(d => { d.animation = animation; });
      },
      setDirection({ direction }: { direction: Direction }) {
        if (DIRECTIONS.includes(direction)) page.update(d => { d.direction = direction; });
      },
      setPeek({ value }: { value: number }) {
        if (!Number.isFinite(value) || value < 0) return;
        page.update(d => { d.peek = Math.round(value); });
      },
      onSwiperChange({ index }: { index: number }) {
        // Friendly log: only emit a line when the index *actually* changes (some
        // race paths in native may still echo same-index transitions on rebuild).
        if (index === page.data.index) return;
        page.update(d => {
          d.index = index;
          d.eventLog = `Showing item ${index + 1}`;
        });
      },
      onSwiperTransitionEnd({ index }: { index: number }) {
        page.update(d => { d.eventLog = `Settled on item ${index + 1}`; });
      },
      onSwiperEndReached() {
        page.update(d => { d.eventLog = "Reached the end"; });
      },
      onSwiperTap({ index }: { index: number }) {
        page.update(d => { d.eventLog = `Tapped item ${index + 1}`; });
      },
      onSwiperVideoEnded({ index }: { index: number }) {
        const items = page.data.items;
        // Native does not auto-advance — JS owns the next-index decision so loop /
        // step semantics are policy-driven, not baked into the native swiper.
        if (items.length === 0) {
          page.update(d => { d.eventLog = "Video finished"; });
          return;
        }
        if (items.length === 1) {
          page.update(d => { d.eventLog = `Video finished (item ${index + 1})`; });
          return;
        }
        const last = items.length - 1;
        const atEnd = index >= last;
        if (atEnd && !page.data.loop) {
          page.update(d => { d.eventLog = `Video finished (last item)`; });
          return;
        }
        const next = atEnd ? 0 : index + 1;
        page.update(d => {
          d.index = next;
          d.eventLog = `Video finished — auto-advancing to item ${next + 1}`;
        });
      },
      onSwiperError({ code }: { code?: string }) {
        page.update(d => { d.eventLog = `Playback error: ${code || "unknown"}`; });
      },
    };
    return { actions };
  },
});

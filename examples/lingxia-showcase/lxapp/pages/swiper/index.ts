import { eventDetail, type NativeEvent } from "../../shared/lib/native-events";

// What the native swiper reports back.
type SwiperEventDetail = { index: number; code: string };
import { errorMessage } from "../../shared/lib/errors";
import { definePage, lx } from '@lingxia/logic';
let nextItemId = 1;
function makeId() {
  const id = `picked-${nextItemId}`;
  nextItemId += 1;
  return id;
}

function clampIndex(value: unknown, count: number) {
  if (count <= 0) return 0;
  if (typeof value !== "number" || !Number.isFinite(value)) return 0;
  return Math.max(0, Math.min(count - 1, Math.trunc(value)));
}



export default definePage({
  data: {
    items: [] as { id: string; type: "image" | "video"; src: string }[],
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
  },
  setup(page) {
    const helpers = {
      async addMedia(mediaType: ("image" | "video")[], count: number) {
        if (page.data.busy) return;
        page.update(d => { d.busy = true; });
        try {
          const result = await lx.pick.media({
            mediaType,
            sourceType: ["album", "camera"],
            count,
            camera: "back",
            maxDurationSeconds: 60,
          });
          if (result.status === 'canceled') {
            page.update(d => { d.busy = false; });
            return;
          }
          const picked = result.files.map((file) => ({
            id: makeId(),
            type: file.type.startsWith("video/") ? "video" : "image",
            src: file.path,
          }));
          const merged = [...(page.data.items || []), ...picked];
          page.update(d => {
            d.items = merged;
            d.eventLog = `added ${picked.length} ${picked.length === 1 ? "item" : "items"}`;
            d.busy = false;
          });
        } catch (error) {
          page.update(d => { d.busy = false; });
          lx.ui.toast({
            title: errorMessage(error, "chooseMedia failed"),
            icon: "none",
          });
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
        const items = page.data.items || [];
        if (items.length === 0) return;
        const current = page.data.index || 0;
        const next = items.filter((_, i) => i !== current);
        const adjusted = clampIndex(current, next.length);
        page.update(d => {
          d.items = next;
          d.index = adjusted;
          d.eventLog = next.length > 0 ? "removed" : "cleared";
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
          d.autoplay = !page.data.autoplay;
          d.eventLog = !page.data.autoplay ? "autoplay on" : "autoplay off";
        });
      },
      toggleLoop() {
        page.update(d => {
          d.loop = !page.data.loop;
          d.eventLog = !page.data.loop ? "loop on" : "loop off";
        });
      },
      toggleDots() {
        page.update(d => { d.dots = !page.data.dots; });
      },
      setObjectFit(params: { fit?: string } = {}) {
        const fit = params?.fit;
        if (fit !== "cover" && fit !== "contain" && fit !== "fill") return;
        page.update(d => { d.objectFit = fit; });
      },
      setAnimation(params: { animation?: string } = {}) {
        const animation = params?.animation;
        if (animation !== "slide" && animation !== "none") return;
        page.update(d => { d.animation = animation; });
      },
      setDirection(params: { direction?: string } = {}) {
        const direction = params?.direction;
        if (direction !== "horizontal" && direction !== "vertical") return;
        page.update(d => { d.direction = direction; });
      },
      setPeek(params: { value?: unknown } = {}) {
        const value = Number(params?.value);
        if (!Number.isFinite(value) || value < 0) return;
        page.update(d => { d.peek = Math.round(value); });
      },
      onSwiperChange(event: NativeEvent<SwiperEventDetail> = {}) {
        const { index } = eventDetail(event);
        if (typeof index !== "number") return;
        // Friendly log: only emit a line when the index *actually* changes (some
        // race paths in native may still echo same-index transitions on rebuild).
        if (index === page.data.index) return;
        page.update(d => {
          d.index = index;
          d.eventLog = `Showing item ${index + 1}`;
        });
      },
      onSwiperTransitionEnd(event: NativeEvent<SwiperEventDetail> = {}) {
        const { index } = eventDetail(event);
        if (typeof index === "number") {
          page.update(d => { d.eventLog = `Settled on item ${index + 1}`; });
        }
      },
      onSwiperEndReached() {
        page.update(d => { d.eventLog = "Reached the end"; });
      },
      onSwiperTap(event: NativeEvent<SwiperEventDetail> = {}) {
        const { index } = eventDetail(event);
        if (typeof index === "number") {
          page.update(d => { d.eventLog = `Tapped item ${index + 1}`; });
        }
      },
      onSwiperVideoEnded(event: NativeEvent<SwiperEventDetail> = {}) {
        const { index } = eventDetail(event);
        const items = page.data.items || [];
        // Native does not auto-advance — JS owns the next-index decision so loop /
        // step semantics are policy-driven, not baked into the native swiper.
        if (typeof index !== "number" || items.length === 0) {
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
      onSwiperError(event: NativeEvent<SwiperEventDetail> = {}) {
        const { code } = eventDetail(event);
        page.update(d => { d.eventLog = `Playback error: ${code || "unknown"}`; });
      },
    };
    return {
      onLoad() {},
      actions,
    };
  },
});

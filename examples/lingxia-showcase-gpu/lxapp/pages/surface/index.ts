import type { PageMessagePort, PageSurface } from "@lingxia/types";
import { definePage } from '@lingxia/types/page';

export default definePage({
  data: {
    queryString: "",
    // Counts how many times the page-level onShow / onHide lifecycle hooks
    // have fired. surface.show()/hide() trigger these the same way navigation
    // away from a regular page would — the hide/show counters bump on each
    // visibility toggle, proving the lifecycle is wired end-to-end.
    showCount: 0,
    hideCount: 0,
    lastLifecycle: "onLoad",
    // Messages the opener pushed down through its PageSurface / PageMessagePort.
    inboundCount: 0,
    lastInbound: "",
  },
  setup(page) {
    let offInbound = [] as (() => void)[];
    const helpers = {
      listenInbound() {
        helpers.stopInbound();
        const receive = (message: unknown) => {
          const next = (page.data.inboundCount || 0) + 1;
          page.update(d => {
            d.inboundCount = next;
            d.lastInbound = JSON.stringify(message ?? null);
          });
        };
        // A page opened as a surface hears its opener on `surface`; one pushed by
        // navigateTo hears it on `opener`. Both are absent for a plain tab/stack page.
        const ports = [page.surface, page.opener].filter(
          (port): port is PageSurface | PageMessagePort => Boolean(port),
        );
        offInbound = ports.map((port) => port.onMessage(receive));
      },
      stopInbound() {
        for (const off of offInbound || []) off();
        offInbound = [];
      },
    };
    const actions = {
      async logSurfaceMessage(params?: { message?: unknown }) {
        const raw =
          params && typeof params.message === "string" ? params.message : "";
        const message = raw.trim();

        if (!message) {
          return;
        }

        const payload = { message, timestamp: Date.now() };
        console.log("surface page message:", payload);
        // Floated or pushed, the opener is the same line; `close` leaves either way.
        if (!page.opener) return;
        page.opener.send(payload);
        await page.close();
      },
      async hideSelf() {
        // Absent unless this page was opened as a surface.
        if (!page.surface) return;
        try {
          await page.surface.hide();
        } catch (error) {
          console.warn("surface.hide failed:", error);
        }
      },
      async closeSelf() {
        if (!page.surface) return;
        try {
          await page.surface.close();
        } catch (error) {
          console.warn("surface.close failed:", error);
        }
      },
    };
    return {
      async onLoad(options) {
        const entries = Object.entries(options || {}).map(([key, value]) => ({
          key,
          value: Array.isArray(value) ? value.join(",") : String(value ?? ""),
        }));

        const queryString = entries.length
          ? entries.map(({ key, value }) => `${key}=${value}`).join("&")
          : "";

        page.update(d => { d.queryString = queryString; });
        helpers.listenInbound();
      },
      onUnload() {
        helpers.stopInbound();
      },
      onShow() {
        console.log("surface page onShow");
        const next = (page.data.showCount || 0) + 1;
        page.update(d => {
          d.showCount = next;
          d.lastLifecycle = `onShow (#${next})`;
        });
      },
      onHide() {
        console.log("surface page onHide");
        const next = (page.data.hideCount || 0) + 1;
        page.update(d => {
          d.hideCount = next;
          d.lastLifecycle = `onHide (#${next})`;
        });
      },
      actions,
    };
  },
});

import type { HostTarget, Lx, PageTarget, TabTarget } from "../src/index.js";

declare const lx: Lx;
declare const page: PageTarget;
declare const tab: TabTarget;

// A page or tab target written for `lx.nav.go` is a notification target as is.
const asHost: HostTarget[] = [page, tab];
void asHost;
void lx.host.notification.show({ title: "t", target: { activate: true } });
void lx.host.notification.show({ title: "t", target: { route: "orders.detail", params: { id: "7" } } });
void lx.host.notification.show({ title: "t", target: { appLink: "https://x.test/o/7" } });
void lx.host.banner.show({ title: "t", actions: [{ id: "o", label: "Open", target: { app: "com.x" } }] });

// @ts-expect-error one destination per target
void lx.host.notification.show({ title: "t", target: { route: "a", appLink: "https://x.test" } });
// @ts-expect-error the `kind` form is not a target
void lx.host.notification.show({ title: "t", target: { kind: "activate" } });
// @ts-expect-error the in-app browser is not a notification target
void lx.host.notification.show({ title: "t", target: { url: "https://x.test" } });
// @ts-expect-error a version pin is not kept on a notification target
void lx.host.notification.show({ title: "t", target: { app: "com.x", version: "1.0.0" } });

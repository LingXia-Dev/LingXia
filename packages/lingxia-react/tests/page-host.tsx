import * as React from "react";
import * as LxReact from "../dist/index.js";
import { useLxHost, useLxPage, type LxHost } from "../dist/index.js";

type PageData = { title: string };
type PageActions = { save: (value: string) => Promise<void> };

// The page mounts once its first state arrived: `data` is whole, no gate.
function Page() {
  const { data, actions } = useLxPage<PageData, PageActions>();
  const title: string = data.title;
  void actions.save(title);
  // @ts-expect-error the page's data is its own type, nothing more
  void data.missing;

  const host: LxHost = useLxHost();
  const workspace = host.sizeClass === "regular" && host.formFactor === "desktop";
  const language: string = host.displayLanguage;
  // @ts-expect-error the size class is `compact` or `regular`, nothing wider
  const wide = host.sizeClass === "wide";
  // @ts-expect-error form factor is a word, not a boolean pair
  void host.isDesktop;
  return <div data-workspace={workspace} data-lang={language} data-wide={wide} />;
}
void Page;

// The page-level hooks are these two; the ones they replace are gone.
// @ts-expect-error folded into useLxHost().displayLanguage
void LxReact.useDisplayLanguage;
// @ts-expect-error folded into useLxHost()
void LxReact.usePlatform;
// @ts-expect-error folded into useLxHost().sizeClass and CSS
void LxReact.useSurfaceContext;
// @ts-expect-error chrome geometry is CSS (--lx-page-chrome-*)
void LxReact.useLxPageChrome;
void React;

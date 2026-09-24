import { useSurfaceContext, type SurfaceContext } from "../dist/index.js";
import type { SurfaceContext as LogicSurfaceContext } from "../../lingxia-types/src/generated/logic.js";

// A View picks its layout from the host's context, with no Logic plumbing and
// no null gate: the host seeds it before the first frame.
function Layout() {
  const context: SurfaceContext = useSurfaceContext();
  const regular: boolean = context.sizeClass === "regular";
  // @ts-expect-error the size class is `compact` or `regular`, nothing wider
  const wide = context.sizeClass === "wide";
  return <div data-regular={regular} data-aside={context.aside} data-width={context.width} data-wide={wide} />;
}
void Layout;

// The View's context and Logic's are one shape; a drift in either fails here.
declare const fromView: SurfaceContext;
declare const fromLogic: LogicSurfaceContext;
const toLogic: LogicSurfaceContext = fromView;
const toView: SurfaceContext = fromLogic;
void [toLogic, toView];

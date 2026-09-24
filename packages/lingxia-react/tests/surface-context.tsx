import { useSurfaceContext, type SurfaceContext } from "../dist/index.js";

// A View picks its layout from the host's context, with no Logic plumbing.
function Layout() {
  const context: SurfaceContext | null = useSurfaceContext();
  if (!context) return null;
  const regular: boolean = context.sizeClass === "regular";
  // @ts-expect-error the size class is `compact` or `regular`, nothing wider
  const wide = context.sizeClass === "wide";
  return <div data-regular={regular} data-aside={context.aside} data-width={context.width} data-wide={wide} />;
}
void Layout;

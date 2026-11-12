import type { Child } from '@lingxia/view';

/** One example on a gallery page: its title above it. */
export function Section({ title, children }: { title: string; children: Child }) {
  return (
    <view className="gap-3">
      <text role="heading" headingLevel={2} className="text-sm font-semibold text-muted">{title}</text>
      {children}
    </view>
  );
}

/** A gallery page: its examples in a scrolling column. */
export function GalleryPage({ children }: { children: Child }) {
  return (
    <page className="bg-canvas">
      <scroll className="grow">
        <view className="p-4 gap-8">{children}</view>
      </scroll>
    </page>
  );
}

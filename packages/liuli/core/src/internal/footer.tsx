import { createContext, useLocal, useView, type Child, type ScrollEdgeEvent } from '@lingxia/view';
import { LayerContext } from './layer';

/** True in a modal footer at compact width: its Buttons share the row equally, the main one last. */
export const SharedRowContext = createContext(false);

/** A Dialog, Drawer or BottomSheet footer. On a phone its actions span the width; wider, they keep theirs at the end. */
export function Footer({ className, children }: { className: string; children: Child }) {
  const shares = useView().env.sizeClass === 'compact';
  return (
    <LayerContext.Provider value="overlay">
      <SharedRowContext.Provider value={shares}>
        <view className={className}>{children}</view>
      </SharedRowContext.Provider>
    </LayerContext.Provider>
  );
}

// A body that runs on under its header or footer shows a hairline there; the negative margin
// gives the line's point back, so the content does not move when it appears.
const lines = {
  start: 'border-t border-text/12 -mt-px',
  end: 'border-b border-text/12 -mb-px',
  both: 'border-y border-text/12 -my-px',
} as const;

/** A modal body's scroll edges: a hairline at the top while content is scrolled under the header, at the bottom while more lies under the footer. */
export function useBodyEdges() {
  // On mount a body reports the edges its content starts at: `start` alone when it runs on, both when it fits.
  // So `end` is unreached unless reported, and nothing is drawn before the first report: a body that fits never flashes a line.
  const edges = useLocal('bodyEdges', { reported: false, start: true, end: false });
  const { reported, start, end } = edges.value;
  const under = !reported ? undefined : !start && !end ? 'both' : !start ? 'start' : !end ? 'end' : undefined;
  return {
    onScrollEdge: ({ edge, at }: ScrollEdgeEvent) => edges.set({ ...edges.value, reported: true, [edge]: at }),
    className: under && lines[under],
  };
}

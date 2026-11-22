import { createContext, useView, type Child } from '@lingxia/view';
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

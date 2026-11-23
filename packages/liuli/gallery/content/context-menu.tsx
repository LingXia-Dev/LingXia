// Generated from the doc file by scripts/generate.mjs; do not edit.
import { Rows, Data } from '../../core/src/ContextMenu/ContextMenu.examples';
import { Lead, PageSection, Stage } from '../shared/Layout';

export function Content() {
  return (
    <>
      <Lead text={"Use a context menu as a shortcut to the actions for one thing."}>
        <Rows />
      </Lead>
      <PageSection title={"Variants"}>
        <Stage caption={"The rows given as data, with a submenu"}>
          <Data />
        </Stage>
      </PageSection>
    </>
  );
}

// Generated from the doc file by scripts/generate.mjs; do not edit.
import { Shortcuts, Hint } from '../../core/src/Kbd/Kbd.examples';
import { Lead, PageSection, Stage } from '../shared/Layout';

export function Content() {
  return (
    <>
      <Lead text={"Use Kbd to show the keys of a shortcut beside the action it runs."}>
        <Shortcuts />
      </Lead>
      <PageSection title={"Variants"}>
        <Stage caption={"In a line of help, one size up"}>
          <Hint />
        </Stage>
      </PageSection>
    </>
  );
}

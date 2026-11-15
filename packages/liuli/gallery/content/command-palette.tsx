// Generated from the doc file by scripts/generate.mjs; do not edit.
import { Commands } from '../../core/src/CommandPalette/CommandPalette.examples';
import { Lead, PageSection, Stage } from '../shared/Layout';

export function Content() {
  return (
    <>
      <Lead text={"Use a command palette to reach any command or place by typing."}>
        <Commands />
      </Lead>
    </>
  );
}

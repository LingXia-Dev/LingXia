// Generated from the doc file by scripts/generate.mjs; do not edit.
import { Archive, Tones, Progress, Faces } from '../../core/src/Toast/Toast.examples';
import { Lead, PageSection, Stage } from '../shared/Layout';

export function Content() {
  return (
    <>
      <Lead text={"Use a toast to confirm an action that worked, or to offer to undo it."}>
        <Archive />
      </Lead>
      <PageSection title={"Variants"}>
        <Stage caption={"Info, success, warning and danger, one at a time"}>
          <Tones />
        </Stage>
        <Stage caption={"A count updated in place"}>
          <Progress />
        </Stage>
        <Stage caption={"A message, success, with Undo, and danger with Retry"}>
          <Faces />
        </Stage>
      </PageSection>
    </>
  );
}

// Generated from the doc file by scripts/generate.mjs; do not edit.
import { Notes, Loading, Labelled, Sizes, StateEmpty, StateFilled, StateNothing, StateDisabled } from '../../core/src/SearchField/SearchField.examples';
import { Lead, PageSection, Stage, StageGrid } from '../shared/Layout';

export function Content() {
  return (
    <>
      <Lead text={"Use a search field above a list or a catalogue, so people can narrow it by typing."}>
        <Notes />
      </Lead>
      <PageSection title={"Variants"}>
        <Stage caption={"A spinner in the magnifier's place while a search runs elsewhere"}>
          <Loading />
        </Stage>
        <Stage caption={"With its label shown, among other fields"}>
          <Labelled />
        </Stage>
      </PageSection>
      <PageSection title={"Sizes"}>
        <Stage caption={"Small, medium and large"}>
          <Sizes />
        </Stage>
      </PageSection>
      <PageSection title={"States"}>
        <StageGrid stack>
          <Stage caption={"Empty"}>
            <StateEmpty />
          </Stage>
          <Stage caption={"With text and a result"}>
            <StateFilled />
          </Stage>
          <Stage caption={"Nothing found"}>
            <StateNothing />
          </Stage>
          <Stage caption={"Disabled"}>
            <StateDisabled />
          </Stage>
        </StageGrid>
      </PageSection>
    </>
  );
}

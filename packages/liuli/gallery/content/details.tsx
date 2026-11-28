// Generated from the doc file by scripts/generate.mjs; do not edit.
import { Single, Accordion, Independent, Sections, WithForm, StateClosed, StateOpen } from '../../core/src/Details/Details.examples';
import { Lead, PageSection, Stage, StageGrid } from '../shared/Layout';

export function Content() {
  return (
    <>
      <Lead text={"Use details for content some people need and most do not: an answer, advanced settings, the fine print."} bare>
        <Single />
      </Lead>
      <PageSection title={"Variants"}>
        <Stage caption={"An accordion: one row open at a time"} bare>
          <Accordion />
        </Stage>
        <Stage caption={"A group whose rows open on their own"} bare>
          <Independent />
        </Stage>
        <Stage caption={"An icon and a second line on each row; the app holds which is open; one row is disabled"} bare>
          <Sections />
        </Stage>
        <Stage caption={"A field inside: what is typed is kept while the row is closed"} bare>
          <WithForm />
        </Stage>
      </PageSection>
      <PageSection title={"States"} note={"Each state is set by its props so you can see it. Nothing here is waiting, missing or broken."}>
        <StageGrid>
          <Stage caption={"State: Closed"}>
            <StateClosed />
          </Stage>
          <Stage caption={"State: Open"}>
            <StateOpen />
          </Stage>
        </StageGrid>
      </PageSection>
    </>
  );
}

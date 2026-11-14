// Generated from the doc file by scripts/generate.mjs; do not edit.
import { Single, Accordion, Independent, StateClosed, StateOpen } from '../../core/src/Details/Details.examples';
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
      </PageSection>
      <PageSection title={"States"}>
        <StageGrid>
          <Stage caption={"Closed"} bare>
            <StateClosed />
          </Stage>
          <Stage caption={"Open"} bare>
            <StateOpen />
          </Stage>
        </StageGrid>
      </PageSection>
    </>
  );
}

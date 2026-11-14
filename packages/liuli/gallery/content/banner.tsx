// Generated from the doc file by scripts/generate.mjs; do not edit.
import { Payment, Tones, StateTitleOnly, StateWithMessage, StateWithAction, StateDismissible, OverList } from '../../core/src/Banner/Banner.examples';
import { Lead, PageSection, Stage, StageGrid } from '../shared/Layout';

export function Content() {
  return (
    <>
      <Lead text={"Use a banner to say something about the page or the account that people should act on or know."}>
        <Payment />
      </Lead>
      <PageSection title={"Variants"}>
        <Stage caption={"Neutral, info, success, warning and danger"}>
          <Tones />
        </Stage>
      </PageSection>
      <PageSection title={"States"}>
        <StageGrid>
          <Stage caption={"A title only"}>
            <StateTitleOnly />
          </Stage>
          <Stage caption={"With a message"}>
            <StateWithMessage />
          </Stage>
          <Stage caption={"With an action"}>
            <StateWithAction />
          </Stage>
          <Stage caption={"Dismissible"}>
            <StateDismissible />
          </Stage>
        </StageGrid>
      </PageSection>
      <PageSection title={"In context"}>
        <Stage caption={"Above the list it is about"}>
          <OverList />
        </Stage>
      </PageSection>
    </>
  );
}

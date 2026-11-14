// Generated from the doc file by scripts/generate.mjs; do not edit.
import { FirstRun, NoResults, Sizes, StateTitleOnly, StateWithPicture, StateWithAction, InCard } from '../../core/src/EmptyState/EmptyState.examples';
import { Lead, PageSection, Stage, StageGrid } from '../shared/Layout';

export function Content() {
  return (
    <>
      <Lead text={"Use an empty state when a list or page has nothing to show yet, to say why and offer the way forward."}>
        <FirstRun />
      </Lead>
      <PageSection title={"Variants"}>
        <Stage caption={"No results, with a way back"}>
          <NoResults />
        </Stage>
      </PageSection>
      <PageSection title={"Sizes"}>
        <Stage caption={"Small for a card or list, medium for a page"}>
          <Sizes />
        </Stage>
      </PageSection>
      <PageSection title={"States"}>
        <StageGrid>
          <Stage caption={"A title only"}>
            <StateTitleOnly />
          </Stage>
          <Stage caption={"With a picture"}>
            <StateWithPicture />
          </Stage>
          <Stage caption={"With an action"}>
            <StateWithAction />
          </Stage>
        </StageGrid>
      </PageSection>
      <PageSection title={"In context"}>
        <Stage caption={"Inside a card"}>
          <InCard />
        </Stage>
      </PageSection>
    </>
  );
}

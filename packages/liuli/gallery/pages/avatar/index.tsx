// Generated from the doc file by scripts/generate.mjs; do not edit.
import { People, Group, Sizes, StatePicture, StateInitials, StateFailed, StateOverflow, WithName } from '../../../core/src/Avatar/Avatar.examples';
import { GalleryPage, Lead, PageSection, Stage, StageGrid } from '../../shared/Layout';

export function render() {
  return (
    <GalleryPage>
      <Lead text={"Use an avatar to show who did something or who is here, next to their name or in a group."}>
        <People />
      </Lead>
      <PageSection title={"Variants"}>
        <Stage caption={"Overlapping, with the rest as a count"}>
          <Group />
        </Stage>
      </PageSection>
      <PageSection title={"Sizes"}>
        <Stage caption={"Extra small, small, medium and large"}>
          <Sizes />
        </Stage>
      </PageSection>
      <PageSection title={"States"}>
        <StageGrid>
          <Stage caption={"A picture"}>
            <StatePicture />
          </Stage>
          <Stage caption={"No picture"}>
            <StateInitials />
          </Stage>
          <Stage caption={"A picture that failed"}>
            <StateFailed />
          </Stage>
          <Stage caption={"Past the group's max"}>
            <StateOverflow />
          </Stage>
        </StageGrid>
      </PageSection>
      <PageSection title={"In context"}>
        <Stage caption={"Beside the name it stands for"}>
          <WithName />
        </Stage>
      </PageSection>
    </GalleryPage>
  );
}

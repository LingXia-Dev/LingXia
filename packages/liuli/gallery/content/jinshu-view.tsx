// Generated from the doc file by scripts/generate.mjs; do not edit.
import { Lesson, Course, Attachments, Resume, Loading, Unavailable } from '../../jinshu/src/JinshuView/JinshuView.examples';
import { Lead, PageSection, Stage, StageGrid } from '../shared/Layout';

export function Content() {
  return (
    <>
      <Lead text={"Use JinshuView to show a structured document for reading: a lesson, an article, a handbook."} bare>
        <Lesson />
      </Lead>
      <PageSection title={"Variants"}>
        <Stage caption={"Video and audio held to a policy"} bare>
          <Course />
        </Stage>
        <Stage caption={"Files, inline or attached"} bare>
          <Attachments />
        </Stage>
      </PageSection>
      <PageSection title={"States"} note={"Each state is set by its props so you can see it. Nothing here is waiting, missing or broken."}>
        <StageGrid stack>
          <Stage caption={"State: Continue where the reader stopped"}>
            <Resume />
          </Stage>
          <Stage caption={"State: Loading"}>
            <Loading />
          </Stage>
          <Stage caption={"State: Unavailable"}>
            <Unavailable />
          </Stage>
        </StageGrid>
      </PageSection>
    </>
  );
}

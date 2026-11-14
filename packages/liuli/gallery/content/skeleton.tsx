// Generated from the doc file by scripts/generate.mjs; do not edit.
import { ListLoading, Shapes, CardLoading } from '../../core/src/Skeleton/Skeleton.examples';
import { Lead, PageSection, Stage } from '../shared/Layout';

export function Content() {
  return (
    <>
      <Lead text={"Use skeletons where content is about to appear, shaped like it, so the page keeps its layout while it loads."}>
        <ListLoading />
      </Lead>
      <PageSection title={"Variants"}>
        <Stage caption={"Lines, a circle and a block"}>
          <Shapes />
        </Stage>
      </PageSection>
      <PageSection title={"In context"}>
        <Stage caption={"A card loading"}>
          <CardLoading />
        </Stage>
      </PageSection>
    </>
  );
}

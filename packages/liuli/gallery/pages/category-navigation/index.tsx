// Generated from the doc file by scripts/generate.mjs; do not edit.
import type { View } from '@lingxia/view';
import { Specimen as TabsSpecimen } from '../../../core/src/Tabs/Tabs.examples';
import { Category, GalleryPage, Tile } from '../../shared/Layout';

type Actions = { open(target: { page: string }): Promise<void> };

export function render({ actions }: View<Actions>) {
  return (
    <GalleryPage>
      <text className="text-body text-muted">{"Ways to move between views on the same page."}</text>
      <Category title={"1 component"}>
        <Tile
          name={"Tabs"}
          tagline={"A few peer views"}
          isWide
          onOpen={() => actions.open({ page: "tabs" })}
          testId={"open-tabs"}
        >
          <TabsSpecimen />
        </Tile>
      </Category>
    </GalleryPage>
  );
}

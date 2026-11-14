// Generated from the doc file by scripts/generate.mjs; do not edit.
import type { View } from '@lingxia/view';
import { Specimen as CardSpecimen } from '../../../core/src/Card/Card.examples';
import { Specimen as ItemSpecimen } from '../../../core/src/Item/Item.examples';
import { Specimen as DetailsSpecimen } from '../../../core/src/Details/Details.examples';
import { Specimen as AvatarSpecimen } from '../../../core/src/Avatar/Avatar.examples';
import { Category, GalleryPage, Tile } from '../../shared/Layout';

type Actions = { open(target: { page: string }): Promise<void> };

export function render({ actions }: View<Actions>) {
  return (
    <GalleryPage>
      <text className="text-body text-muted">{"Containers and rows that hold what the app shows."}</text>
      <Category title={"4 components"}>
        <Tile
          name={"Card"}
          tagline={"Related content"}
          onOpen={() => actions.open({ page: "card" })}
          testId={"open-card"}
        >
          <CardSpecimen />
        </Tile>
        <Tile
          name={"Item"}
          tagline={"One row of a list"}
          onOpen={() => actions.open({ page: "item" })}
          testId={"open-item"}
        >
          <ItemSpecimen />
        </Tile>
        <Tile
          name={"Details"}
          tagline={"More, behind a row"}
          onOpen={() => actions.open({ page: "details" })}
          testId={"open-details"}
        >
          <DetailsSpecimen />
        </Tile>
        <Tile
          name={"Avatar"}
          tagline={"Who, at a glance"}
          onOpen={() => actions.open({ page: "avatar" })}
          testId={"open-avatar"}
        >
          <AvatarSpecimen />
        </Tile>
      </Category>
    </GalleryPage>
  );
}

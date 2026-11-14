// Generated from the doc file by scripts/generate.mjs; do not edit.
import { Specimen as CardSpecimen } from '../../core/src/Card/Card.examples';
import { Specimen as ItemSpecimen } from '../../core/src/Item/Item.examples';
import { Specimen as DetailsSpecimen } from '../../core/src/Details/Details.examples';
import { Specimen as AvatarSpecimen } from '../../core/src/Avatar/Avatar.examples';
import { Specimen as SplitViewSpecimen } from '../../core/src/SplitView/SplitView.examples';
import { Category, Tile } from '../shared/Layout';

export function Content({ onOpen }: { onOpen: (page: string) => void }) {
  return (
    <>
      <text className="text-body text-muted">{"Containers and rows that hold what the app shows."}</text>
      <Category title={"5 components"}>
        <Tile
          name={"Card"}
          tagline={"Related content"}
          onOpen={() => onOpen("card")}
          testId={"open-card"}
        >
          <CardSpecimen />
        </Tile>
        <Tile
          name={"Item"}
          tagline={"One row of a list"}
          onOpen={() => onOpen("item")}
          testId={"open-item"}
        >
          <ItemSpecimen />
        </Tile>
        <Tile
          name={"Details"}
          tagline={"More, behind a row"}
          onOpen={() => onOpen("details")}
          testId={"open-details"}
        >
          <DetailsSpecimen />
        </Tile>
        <Tile
          name={"Avatar"}
          tagline={"Who, at a glance"}
          onOpen={() => onOpen("avatar")}
          testId={"open-avatar"}
        >
          <AvatarSpecimen />
        </Tile>
        <Tile
          name={"SplitView"}
          tagline={"Sidebar and content"}
          isWide
          onOpen={() => onOpen("split-view")}
          testId={"open-split-view"}
        >
          <SplitViewSpecimen />
        </Tile>
      </Category>
    </>
  );
}

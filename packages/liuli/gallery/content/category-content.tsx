// Generated from the doc file by scripts/generate.mjs; do not edit.
import { Specimen as CardSpecimen } from '../../core/src/Card/Card.examples';
import { Specimen as ItemSpecimen } from '../../core/src/Item/Item.examples';
import { Specimen as DetailsSpecimen } from '../../core/src/Details/Details.examples';
import { Specimen as AvatarSpecimen } from '../../core/src/Avatar/Avatar.examples';
import { Specimen as SplitViewSpecimen } from '../../core/src/SplitView/SplitView.examples';
import { Specimen as IconSpecimen } from '../../core/src/Icon/Icon.examples';
import { Specimen as TableSpecimen } from '../../core/src/Table/Table.examples';
import { Specimen as MarkdownSpecimen } from '../../core/src/Markdown/Markdown.examples';
import { Specimen as MetadataListSpecimen } from '../../core/src/MetadataList/MetadataList.examples';
import { Specimen as SectionSpecimen } from '../../core/src/Section/Section.examples';
import { Specimen as DividerSpecimen } from '../../core/src/Divider/Divider.examples';
import { Specimen as KbdSpecimen } from '../../core/src/Kbd/Kbd.examples';
import { Specimen as TimerSpecimen } from '../../core/src/Timer/Timer.examples';
import { Specimen as CodeSpecimen } from '../../core/src/Code/Code.examples';
import { Specimen as TimestampSpecimen } from '../../core/src/Timestamp/Timestamp.examples';
import { Specimen as MessageBubbleSpecimen } from '../../core/src/MessageBubble/MessageBubble.examples';
import { Specimen as ListDetailSpecimen } from '../../core/src/ListDetail/ListDetail.examples';
import { Specimen as SplitSpecimen } from '../../core/src/Split/Split.examples';
import { Category, Tile } from '../shared/Layout';

export function Content({ onOpen }: { onOpen: (page: string) => void }) {
  return (
    <>
      <text className="text-body text-muted">{"Containers, rows and tables that hold what the app shows."}</text>
      <Category title={"18 components"}>
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
          onOpen={() => onOpen("split-view")}
          testId={"open-split-view"}
        >
          <SplitViewSpecimen />
        </Tile>
        <Tile
          name={"Icon"}
          tagline={"Icons by meaning"}
          onOpen={() => onOpen("icon")}
          testId={"open-icon"}
        >
          <IconSpecimen />
        </Tile>
        <Tile
          name={"Table"}
          tagline={"Rows and columns"}
          onOpen={() => onOpen("table")}
          testId={"open-table"}
        >
          <TableSpecimen />
        </Tile>
        <Tile
          name={"Markdown"}
          tagline={"Written text, shown"}
          onOpen={() => onOpen("markdown")}
          testId={"open-markdown"}
        >
          <MarkdownSpecimen />
        </Tile>
        <Tile
          name={"MetadataList"}
          tagline={"Labels and values"}
          onOpen={() => onOpen("metadata-list")}
          testId={"open-metadata-list"}
        >
          <MetadataListSpecimen />
        </Tile>
        <Tile
          name={"Section"}
          tagline={"A titled part"}
          onOpen={() => onOpen("section")}
          testId={"open-section"}
        >
          <SectionSpecimen />
        </Tile>
        <Tile
          name={"Divider"}
          tagline={"A line between"}
          onOpen={() => onOpen("divider")}
          testId={"open-divider"}
        >
          <DividerSpecimen />
        </Tile>
        <Tile
          name={"Kbd"}
          tagline={"Keys of a shortcut"}
          onOpen={() => onOpen("kbd")}
          testId={"open-kbd"}
        >
          <KbdSpecimen />
        </Tile>
        <Tile
          name={"Timer"}
          tagline={"Minutes and seconds"}
          onOpen={() => onOpen("timer")}
          testId={"open-timer"}
        >
          <TimerSpecimen />
        </Tile>
        <Tile
          name={"Code"}
          tagline={"Code, as written"}
          onOpen={() => onOpen("code")}
          testId={"open-code"}
        >
          <CodeSpecimen />
        </Tile>
        <Tile
          name={"Timestamp"}
          tagline={"When it happened"}
          onOpen={() => onOpen("timestamp")}
          testId={"open-timestamp"}
        >
          <TimestampSpecimen />
        </Tile>
        <Tile
          name={"MessageBubble"}
          tagline={"A message"}
          onOpen={() => onOpen("message-bubble")}
          testId={"open-message-bubble"}
        >
          <MessageBubbleSpecimen />
        </Tile>
        <Tile
          name={"ListDetail"}
          tagline={"Master and detail"}
          onOpen={() => onOpen("list-detail")}
          testId={"open-list-detail"}
        >
          <ListDetailSpecimen />
        </Tile>
        <Tile
          name={"Split"}
          tagline={"Primary and aside"}
          onOpen={() => onOpen("split")}
          testId={"open-split"}
        >
          <SplitSpecimen />
        </Tile>
      </Category>
    </>
  );
}

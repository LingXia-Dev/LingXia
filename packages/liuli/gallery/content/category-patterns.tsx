// Generated from the doc file by scripts/generate.mjs; do not edit.
import { SignInSpecimen, SettingsSpecimen, UploadsSpecimen, ListAndDetailSpecimen, DashboardSpecimen, ConversationSpecimen } from '../shared/Specimens';
import { Category, Tile } from '../shared/Layout';

export function Content({ onOpen }: { onOpen: (page: string) => void }) {
  return (
    <>
      <text className="text-body text-muted">{"Whole screens built from the components: forms, settings, lists with their records, and dashboards."}</text>
      <Category title={"6 patterns"}>
        <Tile
          name={"Sign in"}
          tagline={"A form that checks"}
          onOpen={() => onOpen("sign-in")}
          testId={"open-sign-in"}
        >
          <SignInSpecimen />
        </Tile>
        <Tile
          name={"Settings"}
          tagline={"Rows and a confirm"}
          onOpen={() => onOpen("settings")}
          testId={"open-settings"}
        >
          <SettingsSpecimen />
        </Tile>
        <Tile
          name={"Uploads"}
          tagline={"Long-running work"}
          onOpen={() => onOpen("uploads")}
          testId={"open-uploads"}
        >
          <UploadsSpecimen />
        </Tile>
        <Tile
          name={"List & detail"}
          tagline={"A list and a record"}
          onOpen={() => onOpen("list-and-detail")}
          testId={"open-list-and-detail"}
        >
          <ListAndDetailSpecimen />
        </Tile>
        <Tile
          name={"Dashboard"}
          tagline={"Figures and charts"}
          onOpen={() => onOpen("dashboard")}
          testId={"open-dashboard"}
        >
          <DashboardSpecimen />
        </Tile>
        <Tile
          name={"Conversation"}
          tagline={"Messages, a reply"}
          onOpen={() => onOpen("conversation")}
          testId={"open-conversation"}
        >
          <ConversationSpecimen />
        </Tile>
      </Category>
    </>
  );
}

// Generated from the doc file by scripts/generate.mjs; do not edit.
import { SignInSpecimen, SettingsSpecimen, UploadsSpecimen } from '../shared/Specimens';
import { Category, Tile } from '../shared/Layout';

export function Content({ onOpen }: { onOpen: (page: string) => void }) {
  return (
    <>
      <text className="text-body text-muted">{"Whole screens built from the components: forms, settings, lists with their records, and dashboards."}</text>
      <Category title={"3 patterns"}>
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
          isWide
          onOpen={() => onOpen("uploads")}
          testId={"open-uploads"}
        >
          <UploadsSpecimen />
        </Tile>
      </Category>
    </>
  );
}

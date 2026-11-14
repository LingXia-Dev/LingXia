// Generated from the doc file by scripts/generate.mjs; do not edit.
import type { View } from '@lingxia/view';
import { SignInSpecimen, SettingsSpecimen, UploadsSpecimen } from '../../shared/Specimens';
import { Category, GalleryPage, Tile } from '../../shared/Layout';

type Actions = { open(target: { page: string }): Promise<void> };

export function render({ actions }: View<Actions>) {
  return (
    <GalleryPage>
      <text className="text-body text-muted">{"Whole screens built from the components: forms, settings and long-running work."}</text>
      <Category title={"3 patterns"}>
        <Tile
          name={"Sign in"}
          tagline={"A form that checks"}
          onOpen={() => actions.open({ page: "sign-in" })}
          testId={"open-sign-in"}
        >
          <SignInSpecimen />
        </Tile>
        <Tile
          name={"Settings"}
          tagline={"Rows and a confirm"}
          onOpen={() => actions.open({ page: "settings" })}
          testId={"open-settings"}
        >
          <SettingsSpecimen />
        </Tile>
        <Tile
          name={"Uploads"}
          tagline={"Long-running work"}
          isWide
          onOpen={() => actions.open({ page: "uploads" })}
          testId={"open-uploads"}
        >
          <UploadsSpecimen />
        </Tile>
      </Category>
    </GalleryPage>
  );
}

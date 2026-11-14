// Generated from the doc file by scripts/generate.mjs; do not edit.
import type { View } from '@lingxia/view';
import { Specimen as VideoPlayerSpecimen } from '../../../core/src/VideoPlayer/VideoPlayer.examples';
import { Specimen as VideoControlsSpecimen } from '../../../core/src/VideoControls/VideoControls.examples';
import { Category, GalleryPage, Tile } from '../../shared/Layout';

type Actions = { open(target: { page: string }): Promise<void> };

export function render({ actions }: View<Actions>) {
  return (
    <GalleryPage>
      <text className="text-body text-muted">{"Video and its controls."}</text>
      <Category title={"2 components"}>
        <Tile
          name={"VideoPlayer"}
          tagline={"Watch a video"}
          onOpen={() => actions.open({ page: "video-player" })}
          testId={"open-video-player"}
        >
          <VideoPlayerSpecimen />
        </Tile>
        <Tile
          name={"VideoControls"}
          tagline={"Drive a video"}
          onOpen={() => actions.open({ page: "video-controls" })}
          testId={"open-video-controls"}
        >
          <VideoControlsSpecimen />
        </Tile>
      </Category>
    </GalleryPage>
  );
}

// Generated from the doc file by scripts/generate.mjs; do not edit.
import { Specimen as VideoPlayerSpecimen } from '../../core/src/VideoPlayer/VideoPlayer.examples';
import { Specimen as VideoControlsSpecimen } from '../../core/src/VideoControls/VideoControls.examples';
import { Category, Tile } from '../shared/Layout';

export function Content({ onOpen }: { onOpen: (page: string) => void }) {
  return (
    <>
      <text className="text-body text-muted">{"Video and its controls."}</text>
      <Category title={"2 components"}>
        <Tile
          name={"VideoPlayer"}
          tagline={"Watch a video"}
          onOpen={() => onOpen("video-player")}
          testId={"open-video-player"}
        >
          <VideoPlayerSpecimen />
        </Tile>
        <Tile
          name={"VideoControls"}
          tagline={"Drive a video"}
          onOpen={() => onOpen("video-controls")}
          testId={"open-video-controls"}
        >
          <VideoControlsSpecimen />
        </Tile>
      </Category>
    </>
  );
}

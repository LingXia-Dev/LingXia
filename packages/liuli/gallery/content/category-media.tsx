// Generated from the doc file by scripts/generate.mjs; do not edit.
import { Specimen as VideoPlayerSpecimen } from '../../core/src/VideoPlayer/VideoPlayer.examples';
import { Specimen as VideoControlsSpecimen } from '../../core/src/VideoControls/VideoControls.examples';
import { Specimen as CarouselSpecimen } from '../../core/src/Carousel/Carousel.examples';
import { Specimen as ThumbnailSpecimen } from '../../core/src/Thumbnail/Thumbnail.examples';
import { Category, Tile } from '../shared/Layout';

export function Content({ onOpen }: { onOpen: (page: string) => void }) {
  return (
    <>
      <text className="text-body text-muted">{"Video and its controls."}</text>
      <Category title={"4 components"}>
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
        <Tile
          name={"Carousel"}
          tagline={"Swipe through a few"}
          onOpen={() => onOpen("carousel")}
          testId={"open-carousel"}
        >
          <CarouselSpecimen />
        </Tile>
        <Tile
          name={"Thumbnail"}
          tagline={"A small picture"}
          onOpen={() => onOpen("thumbnail")}
          testId={"open-thumbnail"}
        >
          <ThumbnailSpecimen />
        </Tile>
      </Category>
    </>
  );
}

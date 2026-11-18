// Generated from the doc file by scripts/generate.mjs; do not edit.
import { Specimen as VideoPlayerSpecimen } from '../../core/src/VideoPlayer/VideoPlayer.examples';
import { Specimen as VideoControlsSpecimen } from '../../core/src/VideoControls/VideoControls.examples';
import { Specimen as CarouselSpecimen } from '../../core/src/Carousel/Carousel.examples';
import { Specimen as ThumbnailSpecimen } from '../../core/src/Thumbnail/Thumbnail.examples';
import { Specimen as LightboxSpecimen } from '../../core/src/Lightbox/Lightbox.examples';
import { Specimen as WebViewSpecimen } from '../../core/src/WebView/WebView.examples';
import { Category, Tile } from '../shared/Layout';

export function Content({ onOpen }: { onOpen: (page: string) => void }) {
  return (
    <>
      <text className="text-body text-muted">{"Video and its controls."}</text>
      <Category title={"6 components"}>
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
        <Tile
          name={"Lightbox"}
          tagline={"A picture, whole"}
          onOpen={() => onOpen("lightbox")}
          testId={"open-lightbox"}
        >
          <LightboxSpecimen />
        </Tile>
        <Tile
          name={"WebView"}
          tagline={"Embed a web page"}
          onOpen={() => onOpen("web-view")}
          testId={"open-web-view"}
        >
          <WebViewSpecimen />
        </Tile>
      </Category>
    </>
  );
}

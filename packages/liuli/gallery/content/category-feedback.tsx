// Generated from the doc file by scripts/generate.mjs; do not edit.
import { Specimen as BadgeSpecimen } from '../../core/src/Badge/Badge.examples';
import { Specimen as ProgressBarSpecimen } from '../../core/src/ProgressBar/ProgressBar.examples';
import { Specimen as BannerSpecimen } from '../../core/src/Banner/Banner.examples';
import { Specimen as EmptyStateSpecimen } from '../../core/src/EmptyState/EmptyState.examples';
import { Specimen as SkeletonSpecimen } from '../../core/src/Skeleton/Skeleton.examples';
import { Specimen as ToastSpecimen } from '../../core/src/Toast/Toast.examples';
import { Specimen as CircularProgressSpecimen } from '../../core/src/CircularProgress/CircularProgress.examples';
import { Specimen as StatusDotSpecimen } from '../../core/src/StatusDot/StatusDot.examples';
import { Category, Tile } from '../shared/Layout';

export function Content({ onOpen }: { onOpen: (page: string) => void }) {
  return (
    <>
      <text className="text-body text-muted">{"Status, progress and confirmation, from a badge to a toast."}</text>
      <Category title={"8 components"}>
        <Tile
          name={"Badge"}
          tagline={"A status label"}
          onOpen={() => onOpen("badge")}
          testId={"open-badge"}
        >
          <BadgeSpecimen />
        </Tile>
        <Tile
          name={"ProgressBar"}
          tagline={"Progress, or a wait"}
          onOpen={() => onOpen("progress-bar")}
          testId={"open-progress-bar"}
        >
          <ProgressBarSpecimen />
        </Tile>
        <Tile
          name={"Banner"}
          tagline={"Stays until fixed"}
          onOpen={() => onOpen("banner")}
          testId={"open-banner"}
        >
          <BannerSpecimen />
        </Tile>
        <Tile
          name={"EmptyState"}
          tagline={"Nothing here yet"}
          onOpen={() => onOpen("empty-state")}
          testId={"open-empty-state"}
        >
          <EmptyStateSpecimen />
        </Tile>
        <Tile
          name={"Skeleton"}
          tagline={"Content on its way"}
          onOpen={() => onOpen("skeleton")}
          testId={"open-skeleton"}
        >
          <SkeletonSpecimen />
        </Tile>
        <Tile
          name={"Toast"}
          tagline={"Just happened"}
          onOpen={() => onOpen("toast")}
          testId={"open-toast"}
        >
          <ToastSpecimen />
        </Tile>
        <Tile
          name={"CircularProgress"}
          tagline={"Progress as a ring"}
          onOpen={() => onOpen("circular-progress")}
          testId={"open-circular-progress"}
        >
          <CircularProgressSpecimen />
        </Tile>
        <Tile
          name={"StatusDot"}
          tagline={"A dot and a word"}
          onOpen={() => onOpen("status-dot")}
          testId={"open-status-dot"}
        >
          <StatusDotSpecimen />
        </Tile>
      </Category>
    </>
  );
}

// Generated from the doc file by scripts/generate.mjs; do not edit.
import type { View } from '@lingxia/view';
import { Specimen as BadgeSpecimen } from '../../../core/src/Badge/Badge.examples';
import { Specimen as ProgressBarSpecimen } from '../../../core/src/ProgressBar/ProgressBar.examples';
import { Specimen as BannerSpecimen } from '../../../core/src/Banner/Banner.examples';
import { Specimen as EmptyStateSpecimen } from '../../../core/src/EmptyState/EmptyState.examples';
import { Specimen as SkeletonSpecimen } from '../../../core/src/Skeleton/Skeleton.examples';
import { Specimen as ToastSpecimen } from '../../../core/src/Toast/Toast.examples';
import { Category, GalleryPage, Tile } from '../../shared/Layout';

type Actions = { open(target: { page: string }): Promise<void> };

export function render({ actions }: View<Actions>) {
  return (
    <GalleryPage>
      <text className="text-body text-muted">{"Status, progress and confirmation, from a badge to a toast."}</text>
      <Category title={"6 components"}>
        <Tile
          name={"Badge"}
          tagline={"A status label"}
          onOpen={() => actions.open({ page: "badge" })}
          testId={"open-badge"}
        >
          <BadgeSpecimen />
        </Tile>
        <Tile
          name={"ProgressBar"}
          tagline={"Progress, or a wait"}
          onOpen={() => actions.open({ page: "progress-bar" })}
          testId={"open-progress-bar"}
        >
          <ProgressBarSpecimen />
        </Tile>
        <Tile
          name={"Banner"}
          tagline={"Stays until fixed"}
          onOpen={() => actions.open({ page: "banner" })}
          testId={"open-banner"}
        >
          <BannerSpecimen />
        </Tile>
        <Tile
          name={"EmptyState"}
          tagline={"Nothing here yet"}
          onOpen={() => actions.open({ page: "empty-state" })}
          testId={"open-empty-state"}
        >
          <EmptyStateSpecimen />
        </Tile>
        <Tile
          name={"Skeleton"}
          tagline={"Content on its way"}
          onOpen={() => actions.open({ page: "skeleton" })}
          testId={"open-skeleton"}
        >
          <SkeletonSpecimen />
        </Tile>
        <Tile
          name={"Toast"}
          tagline={"Just happened"}
          onOpen={() => actions.open({ page: "toast" })}
          testId={"open-toast"}
        >
          <ToastSpecimen />
        </Tile>
      </Category>
    </GalleryPage>
  );
}

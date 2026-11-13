import { Card } from '../Card/Card';
import { Skeleton, SkeletonGroup } from './Skeleton';

/** A list loading: rows shaped as the rows that will arrive, named once as what is loading. */
export function ListLoading() {
  return (
    <SkeletonGroup label="Loading notes" className="gap-0 overflow-hidden rounded-container border border-text/10 bg-surface divide-y divide-text/8" data-testid="loading-notes">
      {['a', 'b', 'c'].map(key => (
        <view key={key} className="flex-row items-center gap-3 px-4 py-3">
          <Skeleton shape="circle" className="size-10 shrink-0" />
          <view className="grow basis-0 gap-1">
            <Skeleton className="w-2/3" />
            <Skeleton className="w-1/3" />
          </view>
        </view>
      ))}
    </SkeletonGroup>
  );
}

/** A card loading: a picture, a heading and a paragraph. */
export function CardLoading() {
  return (
    <Card>
      <SkeletonGroup label="Loading the article" data-testid="loading-article">
        <Skeleton shape="block" className="h-32 self-stretch" />
        <Skeleton className="w-1/2" />
        <Skeleton lines={3} data-testid="paragraph" />
      </SkeletonGroup>
    </Card>
  );
}

/** The home tile. */
export function Specimen() {
  return (
    <view className="w-28 flex-row items-center gap-2">
      <Skeleton shape="circle" className="size-8 shrink-0" />
      <view className="grow basis-0">
        <Skeleton lines={2} />
      </view>
    </view>
  );
}

export function Shapes() {
  return (
    <view className="gap-4">
      <Skeleton lines={2} />
      <view className="flex-row items-center gap-4">
        <Skeleton shape="circle" className="size-12" />
        <Skeleton shape="block" className="h-12 grow basis-0" />
      </view>
    </view>
  );
}

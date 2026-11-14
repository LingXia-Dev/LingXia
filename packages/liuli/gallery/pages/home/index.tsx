import type { View } from '@lingxia/view';
import { Content } from '../../content/home';
import { GalleryPage } from '../../shared/Layout';
import { Workspace } from '../../shared/Workspace';

type Actions = {
  open(target: { page: string }): Promise<void>;
  select(target: { page: string }): Promise<void>;
};

// A phone opens each page on its own; a wide window shows them all in one workspace.
export function render({ env, data, actions }: View<Actions, { page: string }>) {
  if (env.sizeClass === 'regular') return <Workspace page={data.page} onSelect={page => void actions.select({ page })} />;
  return (
    <GalleryPage>
      <Content onOpen={page => actions.open({ page })} />
    </GalleryPage>
  );
}

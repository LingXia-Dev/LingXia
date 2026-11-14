// Generated from the doc file by scripts/generate.mjs; do not edit.
import type { View } from '@lingxia/view';
import { Content } from '../../content/category-feedback';
import { GalleryPage } from '../../shared/Layout';

type Actions = { open(target: { page: string }): Promise<void> };

export function render({ actions }: View<Actions>) {
  return (
    <GalleryPage>
      <Content onOpen={page => actions.open({ page })} />
    </GalleryPage>
  );
}

import '../../theme/index.css';
import { Page } from '../../shared/ui';
import { useSay } from '../../shared/view-lab/demo';
import { labels } from '../../shared/view-lab/text';

// The page the Motion group pushes to show a page transition.
export function render() {
  const t = useSay();
  return (
    <Page testId="motion-target">
      <text data-testid="motion-target-body" className="px-1 text-sm text-muted">{t(labels.targetBody)}</text>
    </Page>
  );
}

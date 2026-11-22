import { Said, unused } from '../example';
import { IconButton } from '../IconButton/IconButton';
import { TopBar } from './TopBar';

/** A page header with a title and a trailing action. */
export function StandardTopBar() {
  return (
    <Said>
      {say => (
        <view className="overflow-hidden rounded-container border border-text/12">
          <TopBar
            title="Orders"
            trailing={<IconButton icon="add" label="New order" onClick={() => say('Created new order')} />}
            data-testid="orders-topbar"
          />
        </view>
      )}
    </Said>
  );
}

/** Tile specimen showing a centered page title and trailing icon button. */
export function Specimen() {
  return (
    <view className="w-full overflow-hidden rounded-control border border-text/12 bg-surface">
      <TopBar
        title="Orders"
        trailing={<IconButton icon="add" size="sm" label="New order" isDisabled onClick={unused} />}
      />
    </view>
  );
}

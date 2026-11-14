import { useLocal, type Child, type VectorPath } from '@lingxia/view';
import { Avatar } from '@liulidesign/core/Avatar';
import { Badge } from '@liulidesign/core/Badge';
import { Button } from '@liulidesign/core/Button';
import { Dialog } from '@liulidesign/core/Dialog';
import { DropdownMenu, DropdownMenuDivider, DropdownMenuItem, DropdownMenuRadioGroup, DropdownMenuRadioItem } from '@liulidesign/core/DropdownMenu';
import { Item, ItemList } from '@liulidesign/core/Item';
import { RadioGroup } from '@liulidesign/core/RadioGroup';
import { Slider } from '@liulidesign/core/Slider';
import { Switch } from '@liulidesign/core/Switch';

const dots: VectorPath[] = [{ d: 'M5 12h.01M12 12h.01M19 12h.01', fill: 'none', stroke: 'currentColor', strokeWidth: 3, strokeLinecap: 'round' }];
const laptop: VectorPath[] = [{ d: 'M5 6.5h14v9H5zM3 18.5h18', fill: 'none', stroke: 'currentColor', strokeWidth: 1.8, strokeLinecap: 'round', strokeLinejoin: 'round' }];
const phone: VectorPath[] = [{ d: 'M8 3.5h8a1 1 0 0 1 1 1v15a1 1 0 0 1-1 1H8a1 1 0 0 1-1-1v-15a1 1 0 0 1 1-1zM11 17.5h2', fill: 'none', stroke: 'currentColor', strokeWidth: 1.8, strokeLinecap: 'round', strokeLinejoin: 'round' }];

const plans = [
  { value: 'Free', label: 'Free', description: 'Up to 3 notebooks, on one device' },
  { value: 'Pro', label: 'Pro · €4 a month', description: 'Unlimited notebooks, sync across devices' },
  { value: 'Team', label: 'Team · €8 a person', description: 'Shared notebooks and admin controls' },
];

const appearances = ['System', 'Light', 'Dark'];
const digests = ['Daily', 'Weekly', 'Never'];

const initialDevices = [
  { id: 'mac', name: 'MacBook Air', detail: 'This device', icon: laptop },
  { id: 'iphone', name: 'iPhone 15', detail: 'Active today', icon: phone },
  { id: 'tablet', name: 'MatePad 11', detail: 'Active 3 days ago', icon: phone },
];

/** One titled list of rows, the way a settings screen reads; the heading names the list. */
function Group({ title, footer, children }: { title: string; footer?: string; children: Child }) {
  return (
    <view className="gap-2">
      <text role="heading" headingLevel={2} className="text-footnote font-medium text-muted">{title}</text>
      <ItemList label={title}>{children}</ItemList>
      {footer !== undefined && <text className="text-footnote text-muted">{footer}</text>}
    </view>
  );
}

/** A row that sets a value: the label and its help on the left, the current choice as a menu on the right. */
function ChoiceRow({ label, help, value, options, onChange, testId }: { label: string; help: string; value: string; options: string[]; onChange: (next: string) => void; testId: string }) {
  return (
    <Item
      label={label}
      description={help}
      endContent={
        <DropdownMenu label={label} value={value} variant="ghost" size="sm" alignment="end" data-testid={testId}>
          <DropdownMenuRadioGroup value={value} onChange={onChange}>
            {options.map(option => <DropdownMenuRadioItem key={option} value={option} label={option} data-testid={`${testId}-${option.toLowerCase()}`} />)}
          </DropdownMenuRadioGroup>
        </DropdownMenu>
      }
    />
  );
}

export function Content() {
  const appearance = useLocal('appearance', 'System');
  const digest = useLocal('digest', 'Weekly');
  const textSize = useLocal('textSize', 3);
  const plan = useLocal<string | null>('plan', 'Pro');
  const cellular = useLocal('cellular', false);
  const devices = useLocal('devices', initialDevices.map(device => device.id));
  const confirming = useLocal('confirming', false);
  const scheduled = useLocal('scheduled', false);

  return (
    <>
      <Group title="Account">
        <Item
          label="Ana Lima"
          description="ana@example.com"
          startContent={<Avatar name="Ana Lima" isDecorative />}
          endContent={<Badge label={plan.value ?? 'Free'} tone="accent" />}
          data-testid="account"
        />
      </Group>

      <view className="gap-2">
        <text role="heading" headingLevel={2} className="text-footnote font-medium text-muted">Plan</text>
        <RadioGroup bind={plan} label="Plan" isLabelHidden options={plans} variant="card" data-testid="plan" />
      </view>

      <Group title="General">
        <ChoiceRow
          label="Appearance"
          help="System follows the device"
          value={appearance.value}
          options={appearances}
          onChange={next => appearance.set(next)}
          testId="appearance"
        />
        <ChoiceRow
          label="Email digest"
          help="What changed in your notes"
          value={digest.value}
          options={digests}
          onChange={next => digest.set(next)}
          testId="digest"
        />
        <Slider
          bind={textSize}
          label="Text size"
          valueLabel={['Smallest', 'Small', 'Default', 'Large', 'Largest'][textSize.value - 1]}
          min={1}
          max={5}
          variant="row"
          data-testid="text-size"
        />
        <Item label="Offline notes" endContent={<Badge label="Synced" tone="success" size="sm" />} />
        <Switch bind={cellular} label="Sync over mobile data" variant="row" data-testid="cellular" />
      </Group>

      <Group title="Signed-in devices" footer="Signing a device out removes its offline copy.">
        {initialDevices
          .filter(device => devices.value.includes(device.id))
          .map(device => (
            <Item
              key={device.id}
              label={device.name}
              description={device.detail}
              startContent={<vector aria-hidden paths={device.icon} viewBox={[0, 0, 24, 24]} className="size-5 text-muted" />}
              endContent={
                <DropdownMenu
                  label={`${device.name} actions`}
                  isLabelHidden
                  icon={<vector aria-hidden paths={dots} viewBox={[0, 0, 24, 24]} className="size-5" />}
                  variant="ghost"
                  size="sm"
                  alignment="end"
                  data-testid={`device-${device.id}`}
                >
                  <DropdownMenuItem label="Rename" onClick={() => {}} />
                  <DropdownMenuDivider />
                  <DropdownMenuItem
                    label="Sign out"
                    variant="destructive"
                    isDisabled={device.id === 'mac'}
                    onClick={() => devices.set(devices.value.filter(id => id !== device.id))}
                    data-testid={`sign-out-${device.id}`}
                  />
                </DropdownMenu>
              }
            />
          ))}
      </Group>

      <Group title="Danger zone">
        {scheduled.value ? (
          <Item
            label="Deletion scheduled"
            description="Your account goes in 30 days"
            endContent={<Button label="Cancel" size="sm" onClick={() => scheduled.set(false)} data-testid="cancel-deletion" />}
            data-testid="deletion-scheduled"
          />
        ) : (
          <Item
            label="Delete account"
            description="Removes your notes from every device"
            tone="danger"
            hasChevron
            onClick={() => confirming.set(true)}
            data-testid="delete-account"
          />
        )}
      </Group>

      <Dialog
        isOpen={confirming.value}
        onOpenChange={next => confirming.set(next)}
        label="Delete your account?"
        description="Your notes leave every device after 30 days. Until then you can sign in to cancel."
        size="sm"
        isAlert
        dismissOn="closeRequest"
        data-testid="delete-dialog"
        actions={
          <>
            <Button label="Keep account" onClick={() => confirming.set(false)} data-testid="keep-account" />
            <Button label="Delete" variant="destructive" onClick={() => { scheduled.set(true); confirming.set(false); }} data-testid="confirm-delete" />
          </>
        }
      />
    </>
  );
}

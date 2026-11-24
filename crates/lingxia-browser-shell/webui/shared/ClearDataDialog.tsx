import { t, useLocal, type MessageKey } from '@lingxia/view';
import { Button } from '@liulidesign/core/Button';
import { CheckboxInput } from '@liulidesign/core/CheckboxInput';
import { Dialog } from '@liulidesign/core/Dialog';
import { Select } from '@liulidesign/core/Select';
import { TIME_RANGES, type ClearBrowsingDataInput, type TimeRange } from './privacy';

export interface ClearDataDialogProps {
  isOpen: boolean;
  onOpenChange: (isOpen: boolean) => void;
  /** Clears what was chosen; the dialog closes once it settles. */
  onClear: (input: ClearBrowsingDataInput) => Promise<unknown>;
}

/** The browser's "Clear browsing data" form: a time range and what to clear. */
export function ClearDataDialog({ isOpen, onOpenChange, onClear }: ClearDataDialogProps) {
  const range = useLocal<string>('clearRange', 'last24Hours');
  const history = useLocal('clearHistory', true);
  const cache = useLocal('clearCache', true);
  const siteData = useLocal('clearSiteData', false);
  const nothing = !history.value && !cache.value && !siteData.value;

  return (
    <Dialog
      isOpen={isOpen}
      onOpenChange={onOpenChange}
      label={t('privacy.clear.title')}
      size="sm"
      data-testid="clear-data-dialog"
      actions={
        <>
          <Button label={t('common.cancel')} onClick={() => onOpenChange(false)} />
          <Button
            label={t('privacy.clear.confirm')}
            variant="destructive"
            isDisabled={nothing}
            disabledMessage={t('privacy.clear.nothingSelected')}
            data-testid="clear-data-confirm"
            clickAction={async () => {
              await onClear({ timeRange: range.value as TimeRange, history: history.value, cache: cache.value, siteData: siteData.value });
              onOpenChange(false);
            }}
          />
        </>
      }
    >
      <view className="gap-4">
        <Select
          label={t('privacy.timeRange')}
          bind={range}
          items={TIME_RANGES.map((value) => ({ value, label: t(`privacy.range.${value}` as MessageKey) }))}
        />
        <view className="gap-2">
          <CheckboxInput label={t('privacy.history')} bind={history} />
          <CheckboxInput label={t('privacy.cache')} bind={cache} />
          <CheckboxInput label={t('privacy.siteData')} bind={siteData} />
        </view>
      </view>
    </Dialog>
  );
}

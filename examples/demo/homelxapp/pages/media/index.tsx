import React from 'react';
import '../../tailwind.css';

const SOURCE_OPTIONS = [
  { key: 'album', label: 'Album' },
  { key: 'camera', label: 'Camera' },
  { key: 'either', label: 'Album or Camera' },
];


const COUNT_OPTIONS = Array.from({ length: 9 }, (_, index) => {
  const value = index + 1;
  return {
    key: String(value),
    label: String(value),
    value,
  };
});

const CAMERA_OPTIONS = [
  { key: 'back', label: 'Rear Camera' },
  { key: 'front', label: 'Front Camera' },
];

const DURATION_OPTIONS = [
  { key: '15', label: '15 seconds', value: 15 },
  { key: '30', label: '30 seconds', value: 30 },
  { key: '60', label: '60 seconds', value: 60 },
];

type MediaItem = {
  path: string;
  type: 'image' | 'video';
};

type PageData = {
  mediaType?: 'image' | 'video' | 'scanCode';
  selectedMedia?: MediaItem[];
  isRunning?: boolean;
  countLimit?: number;
  sourceKey?: string;
  countKey?: string;
  cameraKey?: string;
  durationKey?: string;
  durationValue?: number;
  emptyHint?: string;
  previewHint?: string;
  galleryHint?: string;
  headerSubtitle?: string;
  addLabel?: string;
  scanResult?: string;
  scanType?: string;
  scanBusy?: boolean;
  scanOnlyCamera?: boolean;
  scanTypeKey?: string;
};

type PageActions = {
  data: PageData;
  launchMediaDemo(): void;
  previewSelectedMedia(payload: { index?: number; path?: string; item?: MediaItem }): void;
  openSourcePicker?(): void;
  openCountPicker?(): void;
  openCameraPicker?(): void;
  openDurationPicker?(): void;
  openScanSourcePicker?(): void;
  openScanTypePicker?(): void;
  startScan?(): void;
};

declare function useLingXia(): PageActions;

export default function MediaPage() {
  const {
    data,
    launchMediaDemo,
    previewSelectedMedia,
    openSourcePicker,
    openCountPicker,
    openCameraPicker,
    openDurationPicker,
    openScanSourcePicker,
    openScanTypePicker,
    startScan,
  } = useLingXia();

  const mediaTypeInput = data?.mediaType || 'image';
  const mediaType = mediaTypeInput === 'video'
    ? 'video'
    : (mediaTypeInput === 'scanCode')
      ? 'scanCode'
      : 'image';
  const selectedMedia: MediaItem[] = Array.isArray(data?.selectedMedia)
    ? (data?.selectedMedia as MediaItem[])
    : [];
  const isRunning = Boolean(data?.isRunning);
  const sourceKey = data?.sourceKey || SOURCE_OPTIONS[0].key;
  const countKey = data?.countKey || COUNT_OPTIONS[COUNT_OPTIONS.length - 1].key;
  const cameraKey = data?.cameraKey || CAMERA_OPTIONS[0].key;
  const durationKey = data?.durationKey || DURATION_OPTIONS[DURATION_OPTIONS.length - 1].key;

  const sourceOption =
    SOURCE_OPTIONS.find((option) => option.key === sourceKey) || SOURCE_OPTIONS[0];
  const countOption =
    COUNT_OPTIONS.find((option) => option.key === countKey) ||
    COUNT_OPTIONS[COUNT_OPTIONS.length - 1];
  const cameraOption =
    CAMERA_OPTIONS.find((option) => option.key === cameraKey) || CAMERA_OPTIONS[0];
  const durationOption =
    DURATION_OPTIONS.find((option) => option.key === durationKey) ||
    DURATION_OPTIONS[DURATION_OPTIONS.length - 1];

  const sourceLabel = sourceOption.label;
  const countLabel = countOption.label;
  const cameraLabel = cameraOption.label;
  const durationLabel = durationOption.label;

  const countLimit =
  typeof data?.countLimit === 'number' ? data.countLimit : countOption.value ?? 0;
  const counterText = countLimit ? `${selectedMedia.length}/${countLimit}` : `${selectedMedia.length}`;

  const isPictureMode = mediaType === 'image';
  const isScanMode = mediaType === 'scanCode';

  const emptyHint = data?.emptyHint || (isPictureMode ? 'Tap + to pick photos.' : 'Tap + to add a video.');
  const previewHint = data?.previewHint || (isPictureMode ? 'Tap a photo to preview.' : 'Tap the clip to preview.');
  const galleryHint = data?.galleryHint || previewHint;
  const headerSubtitle = data?.headerSubtitle || 'choose/previewMedia';

  const scanResult = (typeof data?.scanResult === 'string') ? data?.scanResult : '';
  const scanBusy = Boolean(data?.scanBusy);

  const addLabel = data?.addLabel || (isPictureMode ? 'Add Photo' : 'Add Video');
  const helperText = selectedMedia.length ? previewHint : emptyHint;
  const enforceLimit = isPictureMode
    ? countLimit || Number.POSITIVE_INFINITY
    : 1;
  const canAddMore = selectedMedia.length < enforceLimit;

  const handleChoose = React.useCallback(() => {
    if (!isRunning && canAddMore) {
      launchMediaDemo();
    }
  }, [isRunning, canAddMore, launchMediaDemo]);

  const handlePreview = React.useCallback(
    (item: MediaItem) => {
      previewSelectedMedia({ item });
    },
    [previewSelectedMedia],
  );

  const renderAddTile = () => {
    const baseClass = isPictureMode ? 'aspect-square' : 'h-48';
    const disabled = isRunning || !canAddMore;
    const disabledClasses = disabled ? 'cursor-not-allowed opacity-60' : 'hover:bg-gray-100';
    return (
      <button
        type="button"
        className={`flex w-full flex-col items-center justify-center rounded-lg border border-dashed border-gray-300 bg-gray-50 text-gray-400 ${baseClass} ${disabledClasses}`}
        onClick={handleChoose}
        disabled={disabled}
      >
        <span className="text-3xl leading-none">+</span>
        <span className="mt-2 text-xs uppercase tracking-wide text-gray-400">
          {addLabel}
        </span>
      </button>
    );
  };

  const renderPictureTiles = () => {
    const tiles: React.ReactNode[] = selectedMedia.map((item, index) => (
      <button
        type="button"
        key={`${item.path}-${index}`}
        className="relative aspect-square overflow-hidden rounded-lg border border-gray-200 bg-gray-50"
        onClick={() => handlePreview(item)}
      >
        <img
          src={item.path}
          alt=""
          className="h-full w-full object-cover"
        />
        <div className="absolute inset-x-0 bottom-0 bg-black/50 px-1 py-0.5 text-[10px] text-white truncate">
          {item.path}
        </div>
      </button>
    ));

    if (canAddMore) {
      tiles.push(<div key="add">{renderAddTile()}</div>);
    }

    return <div className="grid grid-cols-3 gap-2">{tiles}</div>;
  };

  const renderVideoTiles = () => {
    return (
      <div className="space-y-3">
        {selectedMedia.map((item, index) => (
          <button
            type="button"
            key={`${item.path}-${index}`}
            className="relative h-48 overflow-hidden rounded-lg border border-gray-200 bg-black"
            onClick={() => handlePreview(item)}
          >
            <video
              src={item.path}
              className="h-full w-full object-cover opacity-90"
              muted
            />
            <div className="absolute inset-0 flex items-center justify-center">
              <div className="flex h-12 w-12 items-center justify-center rounded-full bg-black/60 text-white">
                ▶
              </div>
            </div>
            <div className="absolute inset-x-0 bottom-0 bg-black/50 px-2 py-1 text-[10px] text-white truncate">
              {item.path}
            </div>
          </button>
        ))}
        {canAddMore ? renderAddTile() : null}
      </div>
    );
  };

  const SettingRow: React.FC<{
    label: string;
    value: string;
    onPress?: () => void;
  }> = ({ label, value, onPress }) => {
    const clickable = typeof onPress === 'function';
    return (
      <button
        type="button"
        className={`flex w-full items-center px-5 py-3 text-sm text-left ${
          clickable ? 'text-gray-700 hover:bg-gray-50' : 'text-gray-600 cursor-default'
        }`}
        onClick={clickable ? onPress : undefined}
        disabled={!clickable}
      >
        <span className="text-gray-500 flex-1 pr-3 whitespace-nowrap text-left">{label}</span>
        <span className="font-medium text-gray-900 max-w-[60%] truncate text-right">{value}</span>
      </button>
    );
  };

  const scanSourceLabel = data?.scanOnlyCamera ? 'Camera' : 'Camera & Album';
  const scanTypeKey = data?.scanTypeKey || 'all';
  // Show raw key directly (no conversion): e.g., barCode, qrCode, pdf417
  const scanTypeLabel = String(scanTypeKey);

  const settingRows = isScanMode
    ? [
        { label: 'Source', value: scanSourceLabel, action: openScanSourcePicker },
        { label: 'Scan Type', value: scanTypeLabel, action: openScanTypePicker },
      ]
    : isPictureMode
      ? [
        { label: 'Photo Source', value: sourceLabel, action: openSourcePicker },
        { label: 'Count Limit', value: countLabel, action: openCountPicker },
      ]
      : [
        { label: 'Video Source', value: sourceLabel, action: openSourcePicker },
        { label: 'Camera', value: cameraLabel, action: openCameraPicker },
        { label: 'Duration', value: durationLabel, action: openDurationPicker },
      ];

  const pagePaddingX = isScanMode ? 'px-0' : 'px-4';
  return (
    <div className="min-h-screen bg-gray-100">
      <div className={`${pagePaddingX} py-5 space-y-4`}>
        <div className="bg-white shadow-sm">
          <div className="px-5 py-6 text-center">
            <div className="text-base font-medium text-gray-700">{isScanMode ? 'lx.scanCode' : headerSubtitle}</div>
            <div className="mx-auto mt-3 h-0.5 w-12 bg-gray-200" />
          </div>
          <div className="border-t border-gray-100">
            {settingRows.map(({ label, value, action }, index) => (
              <React.Fragment key={label}>
                <SettingRow label={label} value={value} onPress={action} />
                {index < settingRows.length - 1 ? <div className="h-px bg-gray-100" /> : null}
              </React.Fragment>
            ))}
          </div>
        </div>

        <div className={`space-y-3 bg-white ${isScanMode ? 'p-6 w-full' : 'rounded-xl border border-gray-200 p-4 shadow-sm'}`}>
          {!isScanMode && (
            <div className="flex items-center justify-between text-xs text-gray-500">
              <span>{helperText}</span>
              {selectedMedia.length ? null : null}
            </div>
          )}
          {!isScanMode && countLimit ? (
            <div className="text-xs text-gray-400">Selected {counterText}</div>
          ) : null}
          {!isScanMode && selectedMedia.length ? (
            <div className="text-[10px] text-gray-400">{galleryHint}</div>
          ) : null}

          {isScanMode ? (
            <>
              <div className="space-y-2">
                <div className="text-xs font-semibold uppercase tracking-wide text-gray-500">Scan Result</div>
                <div className="min-h-[7rem] w-full rounded-lg bg-gray-50 px-4 py-3 text-base text-gray-900 break-words">
                  {scanResult}
                </div>
                <div className="text-xs text-gray-400">Type: {typeof data?.scanType === 'string' && data?.scanType ? data.scanType : '--'}</div>
              </div>

              <button
                type="button"
                className={`mt-3 w-full rounded-lg bg-blue-600 py-3 text-sm font-medium text-white shadow-sm transition hover:bg-blue-500 ${scanBusy ? 'opacity-70' : ''}`}
                onClick={() => { startScan(); }}
                disabled={scanBusy}
              >
                {'ScanCode'}
              </button>
            </>
          ) : (
            isPictureMode ? renderPictureTiles() : renderVideoTiles()
          )}
        </div>
      </div>
    </div>
  );
}

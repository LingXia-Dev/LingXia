import React from 'react';
import '../../tailwind.css';
import { LxVideo } from 'lingxia-components/react';

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

type ImageInfoResult = {
  width?: number;
  height?: number;
  type?: string;
  path?: string;
  size?: number;
};

type PageData = {
  mediaType?: 'image' | 'video' | 'scanCode' | 'imageInfo' | 'saveToAlbum';
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
  imageInfoResult?: ImageInfoResult | null;
  imageInfoError?: string;
  compressQuality?: string | number;
  compressedWidth?: string | number;
  compressedHeight?: string | number;
  compressing?: boolean;
  compressResult?: ImageInfoResult | null;
  compressError?: string;
  imageInfoBusy?: boolean;
  saveToAlbumBusy?: boolean;
  saveToAlbumResult?: string;
  saveToAlbumError?: string;
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
  onCompressQualityInput?(event: any): void;
  onCompressedWidthInput?(event: any): void;
  onCompressedHeightInput?(event: any): void;
  pickImageForInfo?(): void;
  pickImageForCompress?(): void;
  compressSelectedImage?(): void;
  previewCompressedImage?(): void;
  captureImageForAlbum?(): void;
  captureVideoForAlbum?(): void;
};

declare function useLingXia(): PageActions;

const Card: React.FC<{ children: React.ReactNode; className?: string; noPadding?: boolean }> = ({
  children,
  className = '',
  noPadding = false
}) => (
  <div className={`w-full bg-white rounded-2xl shadow-sm border border-gray-100 ${noPadding ? '' : 'p-6'} ${className}`}>
    {children}
  </div>
);

const PageHeader: React.FC<{
  title: string;
  subtitle?: string;
  description?: string;
}> = ({ title, subtitle, description }) => (
  <Card className="text-center">
    <div className="space-y-3">
      <h1 className="text-xl font-semibold text-gray-800">{title}</h1>
      {subtitle && (
        <div className="flex items-center justify-center gap-2">
          <div className="h-px w-8 bg-gradient-to-r from-transparent via-blue-400 to-transparent" />
          <p className="text-sm font-medium text-blue-600">{subtitle}</p>
          <div className="h-px w-8 bg-gradient-to-r from-transparent via-blue-400 to-transparent" />
        </div>
      )}
      {description && (
        <p className="text-sm text-gray-500 max-w-md mx-auto leading-relaxed">
          {description}
        </p>
      )}
    </div>
  </Card>
);

const SettingRow: React.FC<{
  label: string;
  value: string;
  onPress?: () => void;
  icon?: string;
}> = ({ label, value, onPress, icon = '›' }) => {
  const clickable = typeof onPress === 'function';
  return (
    <button
      type="button"
      className={`group flex w-full items-center gap-4 px-6 py-4 text-sm transition-all ${
        clickable
          ? 'hover:bg-gradient-to-r hover:from-blue-50/50 hover:to-transparent active:scale-[0.99]'
          : 'cursor-default'
      }`}
      onClick={clickable ? onPress : undefined}
      disabled={!clickable}
    >
      <span className="text-gray-600 font-medium flex-shrink-0">{label}</span>
      <div className="flex-1 border-b border-dashed border-gray-200" />
      <span className="font-semibold text-gray-800 transition-colors group-hover:text-blue-600">
        {value}
      </span>
      {clickable && (
        <span className="text-gray-400 text-lg transition-transform group-hover:translate-x-0.5">
          {icon}
        </span>
      )}
    </button>
  );
};

const Button: React.FC<{
  children: React.ReactNode;
  onClick?: () => void;
  disabled?: boolean;
  variant?: 'primary' | 'secondary' | 'success' | 'danger';
  loading?: boolean;
  fullWidth?: boolean;
  size?: 'sm' | 'md' | 'lg';
}> = ({
  children,
  onClick,
  disabled = false,
  variant = 'primary',
  loading = false,
  fullWidth = false,
  size = 'md'
}) => {
  const baseClasses = 'font-medium rounded-xl transition-all duration-200 shadow-sm active:scale-[0.98] disabled:cursor-not-allowed disabled:opacity-60';

  const sizeClasses = {
    sm: 'px-4 py-2 text-xs',
    md: 'px-5 py-3 text-sm',
    lg: 'px-6 py-4 text-base',
  };

  const variantClasses = {
    primary: 'bg-gradient-to-r from-blue-600 to-blue-500 hover:from-blue-500 hover:to-blue-600 text-white shadow-blue-200',
    secondary: 'bg-gradient-to-r from-gray-600 to-gray-500 hover:from-gray-500 hover:to-gray-600 text-white shadow-gray-200',
    success: 'bg-gradient-to-r from-green-600 to-green-500 hover:from-green-500 hover:to-green-600 text-white shadow-green-200',
    danger: 'bg-gradient-to-r from-red-600 to-red-500 hover:from-red-500 hover:to-red-600 text-white shadow-red-200',
  };

  return (
    <button
      type="button"
      onClick={onClick}
      disabled={disabled || loading}
      className={`${baseClasses} ${sizeClasses[size]} ${variantClasses[variant]} ${fullWidth ? 'w-full' : ''}`}
    >
      {loading ? (
        <span className="flex items-center justify-center gap-2">
          <svg className="animate-spin h-4 w-4" viewBox="0 0 24 24">
            <circle className="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" strokeWidth="4" fill="none" />
            <path className="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z" />
          </svg>
          <span>{children}</span>
        </span>
      ) : children}
    </button>
  );
};

const InfoCard: React.FC<{
  title: string;
  items: { label: string; value: string | number }[];
  footer?: React.ReactNode;
}> = ({ title, items, footer }) => (
  <div className="rounded-xl border border-gray-200 bg-gradient-to-br from-gray-50 to-white p-5 space-y-4">
    <h3 className="text-sm font-semibold text-gray-700 flex items-center gap-2">
      <span className="w-1 h-4 bg-blue-500 rounded-full" />
      {title}
    </h3>
    <div className="space-y-3">
      {items.map(({ label, value }) => (
        <div key={label} className="flex items-center justify-between text-sm">
          <span className="text-gray-600">{label}</span>
          <span className="font-semibold text-gray-800">{value}</span>
        </div>
      ))}
    </div>
    {footer && <div className="pt-4 border-t border-gray-200">{footer}</div>}
  </div>
);

const Input: React.FC<{
  type?: 'text' | 'number';
  value: string;
  onChange: (e: React.ChangeEvent<HTMLInputElement>) => void;
  placeholder?: string;
  label?: string;
  min?: number;
  max?: number;
}> = ({ type = 'text', value, onChange, placeholder, label, min, max }) => (
  <div className="space-y-2">
    {label && <label className="text-xs font-medium text-gray-700">{label}</label>}
    <input
      type={type}
      value={value}
      onChange={onChange}
      placeholder={placeholder}
      min={min}
      max={max}
      className="w-full px-4 py-2.5 text-sm border border-gray-200 rounded-xl bg-white focus:outline-none focus:ring-2 focus:ring-blue-500 focus:border-transparent transition-all"
    />
  </div>
);

const EmptyState: React.FC<{ message: string }> = ({ message }) => (
  <div className="flex flex-col items-center justify-center py-12 text-center">
    <p className="text-sm text-gray-500">{message}</p>
  </div>
);

const formatFileSize = (bytes: number): string => {
  if (bytes === 0) return '0 B';
  const k = 1024;
  const sizes = ['B', 'KB', 'MB', 'GB'];
  const i = Math.floor(Math.log(bytes) / Math.log(k));
  return `${(bytes / Math.pow(k, i)).toFixed(2)} ${sizes[i]}`;
};

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
    onCompressQualityInput,
    onCompressedWidthInput,
    onCompressedHeightInput,
    pickImageForInfo,
    pickImageForCompress,
    compressSelectedImage,
    previewCompressedImage,
    captureImageForAlbum,
    captureVideoForAlbum,
  } = useLingXia();

  const mediaTypeInput = data?.mediaType || 'image';
  const isImageInfoMode = mediaTypeInput === 'imageInfo';
  const isSaveToAlbumMode = mediaTypeInput === 'saveToAlbum';
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

  const sourceOption = SOURCE_OPTIONS.find((option) => option.key === sourceKey) || SOURCE_OPTIONS[0];
  const countOption = COUNT_OPTIONS.find((option) => option.key === countKey) || COUNT_OPTIONS[COUNT_OPTIONS.length - 1];
  const cameraOption = CAMERA_OPTIONS.find((option) => option.key === cameraKey) || CAMERA_OPTIONS[0];
  const durationOption = DURATION_OPTIONS.find((option) => option.key === durationKey) || DURATION_OPTIONS[DURATION_OPTIONS.length - 1];

  const countLimit = typeof data?.countLimit === 'number' ? data.countLimit : countOption.value ?? 0;
  const counterText = countLimit ? `${selectedMedia.length}/${countLimit}` : `${selectedMedia.length}`;

  const isPictureMode = mediaType === 'image' && !isImageInfoMode;
  const isScanMode = mediaType === 'scanCode';
  const isVideoMode = mediaType === 'video';

  const emptyHint = data?.emptyHint || (isPictureMode ? 'Tap + to pick photos.' : 'Tap + to add a video.');
  const previewHint = data?.previewHint || (isPictureMode ? 'Tap a photo to preview.' : 'Tap the clip to preview.');
  const headerSubtitle = data?.headerSubtitle || 'choose/previewMedia';

  const scanResult = (typeof data?.scanResult === 'string') ? data?.scanResult : '';
  const scanBusy = Boolean(data?.scanBusy);

  const addLabel = data?.addLabel || (isPictureMode ? 'Add Photo' : 'Add Video');
  const enforceLimit = isPictureMode ? countLimit || Number.POSITIVE_INFINITY : 1;
  const canAddMore = selectedMedia.length < enforceLimit;

  const imageInfoResult = data?.imageInfoResult || null;
  const imageInfoError = data?.imageInfoError || '';
  const imageInfoBusy = Boolean(data?.imageInfoBusy);

  const rawQuality = data?.compressQuality ?? '80';
  const compressQuality = typeof rawQuality === 'number' ? rawQuality.toString() : rawQuality;
  const rawWidth = data?.compressedWidth ?? '';
  const compressedWidth = typeof rawWidth === 'number' ? rawWidth.toString() : rawWidth || '';
  const rawHeight = data?.compressedHeight ?? '';
  const compressedHeight = typeof rawHeight === 'number' ? rawHeight.toString() : rawHeight || '';
  const compressing = Boolean(data?.compressing);
  const compressResult = data?.compressResult || null;
  const compressError = data?.compressError || '';
  const saveToAlbumBusy = Boolean(data?.saveToAlbumBusy);

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
    const baseClass = isPictureMode ? 'h-32' : 'h-48';
    const disabled = isRunning || !canAddMore;

    return (
      <button
        type="button"
        className={`group flex w-full flex-col items-center justify-center rounded-2xl border-2 border-dashed transition-all ${baseClass} ${
          disabled
            ? 'cursor-not-allowed opacity-40 border-gray-200 bg-gray-50'
            : 'border-blue-300 bg-gradient-to-br from-blue-50 to-indigo-50 hover:border-blue-400 hover:from-blue-100 hover:to-indigo-100 active:scale-[0.98]'
        }`}
        onClick={handleChoose}
        disabled={disabled}
      >
        <span className={`text-5xl leading-none transition-transform group-hover:scale-110 ${disabled ? 'text-gray-400' : 'text-blue-500'}`}>+</span>
        <span className={`mt-3 text-xs font-medium uppercase tracking-wider ${
          disabled ? 'text-gray-400' : 'text-blue-600'
        }`}>
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
        className="group relative h-32 overflow-hidden rounded-2xl border border-gray-200 bg-gray-50 transition-all hover:shadow-lg hover:scale-[1.02] active:scale-[0.98]"
        onClick={() => handlePreview(item)}
      >
        <img
          src={item.path}
          alt=""
          className="h-full w-full object-cover transition-transform group-hover:scale-110"
        />
        <div className="absolute inset-0 bg-gradient-to-t from-black/60 via-transparent to-transparent opacity-0 group-hover:opacity-100 transition-opacity" />
        <div className="absolute inset-x-0 bottom-0 bg-gradient-to-t from-black/80 to-transparent px-3 py-2">
          <div className="text-[10px] text-white/90 truncate font-medium">
            Image {index + 1}
          </div>
        </div>
      </button>
    ));

    if (canAddMore) {
      tiles.push(<div key="add">{renderAddTile()}</div>);
    }

    return <div className="grid grid-cols-3 gap-3">{tiles}</div>;
  };

  const renderVideoTiles = () => {
    return (
      <div className="space-y-4">
        {selectedMedia.map((item, index) => (
          <Card key={`video-${index}`} noPadding className="overflow-hidden">
            <LxVideo
              id={`media-video-${index}`}
              src={item.path}
              controls
              autoplay
              muted
              loop
              style={{ width: '100%', height: '224px', display: 'block', backgroundColor: 'black' }}
            />
            <div className="px-5 py-4 bg-gradient-to-br from-gray-50 to-white">
              <div className="flex items-center justify-between gap-4">
                <div className="flex items-center gap-3 flex-1">
                  <div className="flex items-center justify-center w-10 h-10 rounded-xl bg-gradient-to-br from-blue-50 to-indigo-50">
                    <svg className="w-5 h-5 text-blue-600" fill="currentColor" viewBox="0 0 20 20">
                      <path d="M2 6a2 2 0 012-2h6a2 2 0 012 2v8a2 2 0 01-2 2H4a2 2 0 01-2-2V6zm12.553 1.106A1 1 0 0014 8v4a1 1 0 00.553.894l2 1A1 1 0 0018 13V7a1 1 0 00-1.447-.894l-2 1z" />
                    </svg>
                  </div>
                  <div>
                    <div className="text-sm font-semibold text-gray-800">Video {index + 1}</div>
                    <div className="text-xs text-gray-500 mt-0.5">Tap to preview fullscreen</div>
                  </div>
                </div>
                <Button
                  onClick={() => handlePreview(item)}
                  variant="primary"
                  size="md"
                >
                  Preview
                </Button>
              </div>
            </div>
          </Card>
        ))}
        {canAddMore ? renderAddTile() : null}
      </div>
    );
  };

  const renderImageInfoDemo = () => {
    return (
      <div className="space-y-5">
        <Button
          onClick={() => pickImageForInfo?.()}
          disabled={imageInfoBusy}
          loading={imageInfoBusy}
          fullWidth
        >
          {imageInfoBusy ? 'Getting Info…' : 'Pick Image'}
        </Button>

        {imageInfoError && (
          <div className="flex items-center gap-2 text-sm text-red-600 bg-red-50 px-4 py-3 rounded-xl">
            <span>⚠️</span>
            <span>{imageInfoError}</span>
          </div>
        )}

        {imageInfoResult && (
          <InfoCard
            title="Image Information"
            items={[
              { label: 'Width', value: `${imageInfoResult.width ?? '--'} px` },
              { label: 'Height', value: `${imageInfoResult.height ?? '--'} px` },
              { label: 'Type', value: imageInfoResult.type || '--' },
              { label: 'Size', value: formatFileSize(imageInfoResult.size || 0) },
            ]}
            footer={
              imageInfoResult.path ? (
                <div className="space-y-1">
                  <div className="text-xs font-medium text-gray-700">Path</div>
                  <div className="text-[11px] text-gray-500 break-all bg-gray-100 px-3 py-2 rounded-lg">
                    {imageInfoResult.path}
                  </div>
                </div>
              ) : undefined
            }
          />
        )}
      </div>
    );
  };

  const renderCompressDemo = () => {
    const hasImageInfo = Boolean(imageInfoResult);

    return (
      <div className="space-y-5">
        {!hasImageInfo ? (
          <>
            <Button
              onClick={() => pickImageForInfo?.()}
              disabled={imageInfoBusy}
              loading={imageInfoBusy}
              fullWidth
            >
              {imageInfoBusy ? 'Loading…' : 'Pick Image'}
            </Button>

            {imageInfoError && (
              <div className="flex items-center gap-2 text-sm text-red-600 bg-red-50 px-4 py-3 rounded-xl">
                <span>⚠️</span>
                <span>{imageInfoError}</span>
              </div>
            )}
          </>
        ) : (
          <>
            <InfoCard
              title="Source Image"
              items={[
                { label: 'Dimensions', value: `${imageInfoResult.width} × ${imageInfoResult.height}` },
                { label: 'Type', value: imageInfoResult.type || '--' },
                { label: 'File Size', value: formatFileSize(imageInfoResult.size || 0) },
              ]}
              footer={
                imageInfoResult.path ? (
                  <div className="space-y-1">
                    <div className="text-xs font-medium text-gray-700">Path</div>
                    <div className="text-[11px] text-gray-500 break-all bg-gray-100 px-3 py-2 rounded-lg">
                      {imageInfoResult.path}
                    </div>
                  </div>
                ) : undefined
              }
            />

            <div className="grid grid-cols-3 gap-3">
              <Input
                type="number"
                label="Quality"
                value={compressQuality}
                onChange={(e) => onCompressQualityInput?.({ detail: { value: e.target.value } })}
                min={0}
                max={100}
              />
              <Input
                type="number"
                label="Width"
                value={compressedWidth}
                onChange={(e) => onCompressedWidthInput?.({ detail: { value: e.target.value } })}
                placeholder={String(imageInfoResult.width || '')}
                min={0}
              />
              <Input
                type="number"
                label="Height"
                value={compressedHeight}
                onChange={(e) => onCompressedHeightInput?.({ detail: { value: e.target.value } })}
                placeholder={String(imageInfoResult.height || '')}
                min={0}
              />
            </div>

            <Button
              onClick={() => compressSelectedImage?.()}
              disabled={compressing}
              loading={compressing}
              fullWidth
            >
              {compressing ? 'Compressing…' : 'Compress Image'}
            </Button>

            {compressError && (
              <div className="flex items-center gap-2 text-sm text-red-600 bg-red-50 px-4 py-3 rounded-xl">
                <span>⚠️</span>
                <span>{compressError}</span>
              </div>
            )}

            {compressResult && (
              <div className="space-y-4">
                <InfoCard
                  title="Compressed Image"
                  items={[
                    { label: 'Dimensions', value: `${compressResult.width} × ${compressResult.height}` },
                    { label: 'Type', value: compressResult.type || '--' },
                    { label: 'File Size', value: formatFileSize(compressResult.size || 0) },
                  ]}
                  footer={
                    compressResult.path ? (
                      <div className="space-y-1">
                        <div className="text-xs font-medium text-gray-700">Path</div>
                        <div className="text-[11px] text-gray-500 break-all bg-gray-100 px-3 py-2 rounded-lg">
                          {compressResult.path}
                        </div>
                      </div>
                    ) : undefined
                  }
                />
                <Button
                  onClick={() => previewCompressedImage?.()}
                  variant="secondary"
                  fullWidth
                >
                  Preview Image
                </Button>
              </div>
            )}
          </>
        )}
      </div>
    );
  };

  const renderSaveToAlbumDemo = () => {
    return (
      <div className="space-y-5">
        <div className="text-sm text-gray-600 bg-blue-50 px-4 py-3 rounded-xl border border-blue-100">
          📸 Capture photo or video, then save to album. Check your device album to view saved media.
        </div>

        <div className="grid grid-cols-2 gap-4">
          <Button
            onClick={() => captureImageForAlbum?.()}
            disabled={saveToAlbumBusy}
            loading={saveToAlbumBusy}
            fullWidth
          >
            {saveToAlbumBusy ? 'Saving...' : 'Capture Image'}
          </Button>
          <Button
            onClick={() => captureVideoForAlbum?.()}
            disabled={saveToAlbumBusy}
            loading={saveToAlbumBusy}
            variant="success"
            fullWidth
          >
            {saveToAlbumBusy ? 'Saving...' : 'Capture Video'}
          </Button>
        </div>
      </div>
    );
  };

  const renderScanCodeDemo = () => {
    const scanSourceLabel = data?.scanOnlyCamera ? 'Camera' : 'Camera & Album';
    const scanTypeKey = data?.scanTypeKey || 'all';
    const scanTypeLabel = String(scanTypeKey);

    return (
      <>
        <Card noPadding>
          <div className="divide-y divide-gray-100">
            <SettingRow label="Source" value={scanSourceLabel} onPress={openScanSourcePicker} />
            <SettingRow label="Scan Type" value={scanTypeLabel} onPress={openScanTypePicker} />
          </div>
        </Card>

        <Card>
          <div className="space-y-4">
            <div className="space-y-2">
              <h3 className="text-sm font-semibold text-gray-700 flex items-center gap-2">
                <span className="w-1 h-4 bg-blue-500 rounded-full" />
                Scan Result
              </h3>
              <div className="min-h-[8rem] w-full rounded-xl bg-gradient-to-br from-gray-50 to-gray-100 px-5 py-4 text-base text-gray-900 break-words border border-gray-200 font-mono">
                {scanResult || <span className="text-gray-400 italic">No result yet</span>}
              </div>
              <div className="text-xs text-gray-500 flex items-center gap-2">
                <span className="font-medium">Type:</span>
                <span className="px-2 py-1 bg-gray-100 rounded-md">{typeof data?.scanType === 'string' && data?.scanType ? data.scanType : '--'}</span>
              </div>
            </div>

            <Button
              onClick={() => startScan?.()}
              disabled={scanBusy}
              loading={scanBusy}
              fullWidth
            >
              {scanBusy ? 'Scanning...' : 'Start Scan'}
            </Button>
          </div>
        </Card>
      </>
    );
  };

  const settingRows = isScanMode
    ? []  // Handled separately in renderScanCodeDemo
    : (isImageInfoMode || isSaveToAlbumMode)
      ? []
      : isPictureMode
        ? [
          { label: 'Photo Source', value: sourceOption.label, action: openSourcePicker },
          { label: 'Count Limit', value: countOption.label, action: openCountPicker },
        ]
        : [
          { label: 'Video Source', value: sourceOption.label, action: openSourcePicker },
          { label: 'Camera', value: cameraOption.label, action: openCameraPicker },
          { label: 'Duration', value: durationOption.label, action: openDurationPicker },
        ];

  const getPageInfo = () => {
    if (isScanMode) {
      return {
        title: 'lx.scanCode',
        subtitle: 'QR & Barcode Scanner',
        description: 'Scan QR codes and barcodes using camera or album',
      };
    }
    if (isImageInfoMode) {
      return {
        title: 'lx.getImageInfo / lx.compressImage',
        subtitle: 'Image Tools',
        description: 'Get image info and create compressed copy',
      };
    }
    if (isSaveToAlbumMode) {
      return {
        title: 'lx.saveImageToPhotosAlbum / lx.saveVideoToPhotosAlbum',
        subtitle: 'Save to Album',
        description: 'Capture photo or video and save to device album',
      };
    }
    return {
      title: 'Media Manager',
      subtitle: headerSubtitle,
      description: undefined,
    };
  };

  const pageInfo = getPageInfo();

  return (
    <div className="min-h-screen bg-gradient-to-br from-gray-50 to-gray-100">
      <div className="px-4 py-6 space-y-5">
        <PageHeader
          title={pageInfo.title}
          subtitle={pageInfo.subtitle}
          description={pageInfo.description}
        />

        {isScanMode ? (
          renderScanCodeDemo()
        ) : (
          <>
            {settingRows.length > 0 && (
              <Card noPadding>
                <div className="divide-y divide-gray-100">
                  {settingRows.map(({ label, value, action }) => (
                    <SettingRow key={label} label={label} value={value} onPress={action} />
                  ))}
                </div>
              </Card>
            )}

            <Card>
              {isImageInfoMode ? (
                renderImageInfoDemo()
              ) : isSaveToAlbumMode ? (
                renderSaveToAlbumDemo()
              ) : (
                <div className="space-y-4">
                  <div className="flex items-center justify-between">
                    <div className="text-sm text-gray-600">
                      {selectedMedia.length ? previewHint : emptyHint}
                    </div>
                    {countLimit > 0 && (
                      <div className="px-3 py-1 bg-blue-50 text-blue-600 text-xs font-semibold rounded-full">
                        {counterText}
                      </div>
                    )}
                  </div>

                  {selectedMedia.length === 0 ? (
                    <EmptyState message={emptyHint} />
                  ) : (
                    isPictureMode ? renderPictureTiles() : renderVideoTiles()
                  )}

                  {selectedMedia.length === 0 && renderAddTile()}
                </div>
              )}
            </Card>
          </>
        )}
      </div>
    </div>
  );
}

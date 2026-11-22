import { t, type Child } from '@lingxia/view';
import '../../theme/index.css';
import { said } from '../../shared/lib/said';
import { Button, Card, ErrorState, InfoList, InfoRow, Page, Section } from '../../shared/ui';
import type { View } from './$types';

type Data = Record<string, any>;
type Actions = Record<string, (payload?: unknown) => Promise<unknown>>;
type Pane = { data: Data; actions: Actions };

export function render({ data, actions }: View) {
  const pane = { data: data as Data, actions: actions as Actions };
  if (data.mediaType === 'scanCode' || data.mediaType === 'saveToAlbum') return <Capture {...pane} />;
  if (data.mediaType === 'imageInfo') return <ImageTools {...pane} />;
  if (data.mediaType === 'videoTools') return <VideoTools {...pane} />;
  return <Gallery {...pane} />;
}

function fileSize(value = 0) {
  if (!value) return '0 B';
  const unit = Math.min(3, Math.max(0, Math.floor(Math.log(value) / Math.log(1024))));
  return `${(value / Math.pow(1024, unit)).toFixed(2)} ${['B', 'KB', 'MB', 'GB'][unit]}`;
}
const seconds = (value: number) => (value > 0 ? `${(value / 1000).toFixed(2)} s` : '--');
const dimensions = (value: Data) => `${value.width ?? '--'} × ${value.height ?? '--'}`;

/** A pickable setting: label, current value and a chevron when it can change. */
function Setting({
  label,
  value,
  testId,
  onClick,
  disabled,
}: {
  label: string;
  value: string;
  testId: string;
  onClick: () => unknown;
  disabled?: boolean;
}) {
  return (
    <button
      data-testid={testId}
      aria-label={`${label}: ${value}`}
      disabled={disabled}
      onClick={() => {
        void onClick();
      }}
      className="px-4 py-3.5 rounded-none bg-transparent text-text text-left active:bg-surface-sunken disabled:opacity-60"
    >
      <view className="flex-row items-center gap-4">
        <text className="grow basis-0 text-sm font-medium text-muted">{label}</text>
        <text className="text-sm font-semibold">{value}</text>
        {!disabled && <text className="text-lg text-muted">›</text>}
      </view>
    </button>
  );
}

/** Editable numeric options that report through `on…Input` Logic handlers. */
function Inputs({
  data,
  actions,
  fields,
  columns = 2,
}: Pane & { fields: [key: string, label: string, action: string, placeholder: string][]; columns?: number }) {
  return (
    <view className="grid gap-3" style={{ gridTemplateColumns: columns }}>
      {fields.map(([key, label, action, placeholder]) => (
        <view key={key} className="gap-1.5">
          <text className="text-xs font-medium">{label}</text>
          <input
            data-testid={`media-${key}`}
            aria-label={label}
            value={String(data[key] ?? '')}
            placeholder={placeholder}
            onInput={value => actions[action]({ detail: { value } })}
            className="px-3 py-2.5 text-sm rounded-xl border border-line-strong bg-surface"
          />
        </view>
      ))}
    </view>
  );
}

function Path({ value, testId }: { value: string; testId?: string }) {
  return (
    <text data-testid={testId} className="px-3 py-2 rounded-lg bg-surface-sunken text-[11px] text-muted break-all">
      {value}
    </text>
  );
}

function Capture({ data, actions }: Pane) {
  const scan = data.mediaType === 'scanCode';
  return (
    <Page
      intro={scan ? t('media.scanIntro') : t('media.captureIntro')}
    >
      {scan ? (
        <view className="gap-4">
          <Card className="divide-y divide-line">
            <Setting
              testId="media-openScanSourcePicker"
              label={t('media.source')}
              value={data.scanOnlyCamera ? t('media.camera') : t('media.cameraAlbum')}
              onClick={() => actions.openScanSourcePicker()}
            />
            <Setting
              testId="media-openScanTypePicker"
              label={t('media.scanType')}
              value={String(data.scanTypeKey || 'all')}
              onClick={() => actions.openScanTypePicker()}
            />
          </Card>
          <Section title={t('media.scanResult')}>
            <text
              data-testid="media-scan-result"
              className={
                data.scanResult
                  ? 'min-h-32 px-4 py-3 rounded-xl border border-line bg-surface-sunken font-mono break-all'
                  : 'min-h-32 px-4 py-3 rounded-xl border border-line bg-surface-sunken font-mono break-all text-muted'
              }
            >
              {String(data.scanResult || t('media.noResult'))}
            </text>
            <InfoRow label={t('media.type')} value={data.scanType} testId="media-scan-type" />
            <Button testId="media-startScan" disabled={!!data.scanBusy} onClick={() => actions.startScan()}>
              {data.scanBusy ? t('media.scanning') : t('media.startScan')}
            </Button>
          </Section>
        </view>
      ) : (
        <Section title={t('media.capture')} description={t('media.capturePhotoOrVideo')}>
          <view className="grid gap-3 grid-cols-2">
            <Button testId="media-captureImageForAlbum" disabled={!!data.saveToAlbumBusy} onClick={() => actions.captureImageForAlbum()}>
              {data.saveToAlbumBusy ? t('media.saving') : t('media.captureImage')}
            </Button>
            <Button
              variant="secondary"
              testId="media-captureVideoForAlbum"
              disabled={!!data.saveToAlbumBusy}
              onClick={() => actions.captureVideoForAlbum()}
            >
              {data.saveToAlbumBusy ? t('media.saving') : t('media.captureVideo')}
            </Button>
          </view>
        </Section>
      )}
    </Page>
  );
}

function ImageSummary({ title, info, prefix }: { title: string; info: Data; prefix: string }) {
  return (
    <Section title={title}>
      <InfoList>
        <InfoRow label={t('media.width')} value={info.width || 0} testId={`${prefix}-width`} />
        <InfoRow label={t('media.height')} value={info.height || 0} testId={`${prefix}-height`} />
        <InfoRow label={t('media.type')} value={info.type} testId={`${prefix}-type`} />
        <InfoRow label={t('media.fileSize')} value={fileSize(info.size)} testId={`${prefix}-size`} />
      </InfoList>
      {!!info.path && <Path value={info.path} testId={`${prefix}-path`} />}
    </Section>
  );
}

function ImageTools({ data, actions }: Pane) {
  const source = data.imageInfoResult as Data | null;
  const result = data.compressResult as Data | null;
  return (
    <Page intro={t('media.lxMediaImageinfoAnd')}>
      <Button testId="media-pickImageForInfo" disabled={!!data.imageInfoBusy} onClick={() => actions.pickImageForInfo()}>
        {data.imageInfoBusy ? t('media.gettingInfo') : t('media.pickImage')}
      </Button>
      <ErrorState testId="media-imageInfoError" error={data.imageInfoError ? String(data.imageInfoError) : null} />
      {!!source && (
        <view className="gap-4">
          <ImageSummary title={t('media.sourceImage')} info={source} prefix="media-imageInfoResult" />
          <Section title={t('media.compress')}>
            <Inputs
              data={data}
              actions={actions}
              columns={3}
              fields={[
                ['compressQuality', t('media.quality'), 'onCompressQualityInput', ''],
                ['compressedWidth', t('media.width'), 'onCompressedWidthInput', String(source.width || '')],
                ['compressedHeight', t('media.height'), 'onCompressedHeightInput', String(source.height || '')],
              ]}
            />
            <Button testId="media-compressSelectedImage" disabled={!!data.compressing} onClick={() => actions.compressSelectedImage()}>
              {data.compressing ? t('media.compressing') : t('media.compressImage')}
            </Button>
            <ErrorState testId="media-compressError" error={data.compressError ? String(data.compressError) : null} />
          </Section>
        </view>
      )}
      {!!result && (
        <view className="gap-4">
          <ImageSummary title={t('media.compressedImage')} info={result} prefix="media-compressResult" />
          <Button variant="secondary" testId="media-previewCompressedImage" onClick={() => actions.previewCompressedImage()}>
            {t('media.previewImage')}
          </Button>
        </view>
      )}
    </Page>
  );
}

function Facts({ title, rows, children }: { title: string; rows: [label: string, value: unknown, testId?: string][]; children?: Child }) {
  return (
    <Section title={title}>
      <InfoList>
        {rows.map(([label, value, testId]) => (
          <InfoRow key={label} label={label} value={value} testId={testId} />
        ))}
      </InfoList>
      {children}
    </Section>
  );
}

function VideoTools({ data, actions }: Pane) {
  const info = data.videoInfoResult as Data | null;
  const source = data.thumbnailSourceInfo as Data | null;
  const thumb = data.thumbnailResult as Data | null;
  const result = data.videoCompressResult as Data | null;
  const progress = Math.max(0, Math.min(100, Number(data.videoCompressProgress) || 0));
  const ratio = thumb ? Number(thumb.width || 1) / Math.max(1, Number(thumb.height || 1)) : 1;
  return (
    <Page intro={t('media.lxMediaVideoinfoLx')}>
      <Button testId="media-pickVideoForTools" disabled={!!data.thumbnailBusy || !!data.videoInfoBusy} onClick={() => actions.pickVideoForTools()}>
        {data.thumbnailVideoPath ? t('media.pickAnother') : t('media.pickVideo')}
      </Button>
      {!!data.thumbnailVideoPath && <Path value={String(data.thumbnailVideoPath)} />}
      <ErrorState testId="media-videoInfoError" error={data.videoInfoError ? String(data.videoInfoError) : null} />
      {!!info && (
        <Facts
          title={t('media.uploadPreflightMetadata')}
          rows={[
            [t('media.fileSize'), fileSize(info.size), 'media-videoInfoResult-size'],
            [t('media.resolution'), dimensions(info), 'media-videoInfoResult-resolution'],
            [t('media.duration'), seconds(info.durationMs), 'media-videoInfoResult-durationMs'],
            [t('media.rotation'), info.rotation],
            [t('media.bitrate'), info.bitrate > 0 ? `${Math.round(info.bitrate / 100) / 10} kbps` : '--'],
            ['FPS', info.fps],
            [t('media.container'), info.type || t('media.notReported')],
            [t('media.videoCodec'), info.videoCodec || t('media.notReported')],
            [t('media.hasAudio'), info.hasAudio == null ? t('media.notReported') : info.hasAudio ? t('common.yes') : t('common.no')],
            [t('media.audioCodec'), info.hasAudio === false ? 'N/A' : info.audioCodec || t('media.notReported')],
          ]}
        >
          <text className="text-[11px] text-muted">
            {t('media.trackMetadataIsBest')}
          </text>
          <Path value={String(info.path || '')} />
        </Facts>
      )}
      {!!source && (
        <Facts
          title={t('media.sourceVideo')}
          rows={[
            [t('media.resolution'), dimensions(source)],
            [t('media.duration'), seconds(source.durationMs)],
            ['Type', source.type],
          ]}
        />
      )}
      <Section
        title={t('media.thumbnail')}
        description={t('media.time0IsThe')}
      >
        <Inputs
          data={data}
          actions={actions}
          fields={[
            ['thumbnailQuality', t('media.quality'), 'onThumbnailQualityInput', '80'],
            ['thumbnailTimeMs', t('media.timeMs'), 'onThumbnailTimeInput', '0'],
            ['thumbnailMaxWidth', t('media.maxWidth'), 'onThumbnailMaxWidthInput', source?.width ? t('media.example', { value: source.width }) : t('media.leaveEmpty')],
            ['thumbnailMaxHeight', t('media.maxHeight'), 'onThumbnailMaxHeightInput', source?.height ? t('media.example', { value: source.height }) : t('media.leaveEmpty')],
          ]}
        />
        <Button testId="media-createVideoThumbnail" disabled={!!data.thumbnailBusy} onClick={() => actions.createVideoThumbnail()}>
          {data.thumbnailBusy ? t('media.generating') : t('media.generateThumbnail')}
        </Button>
        <ErrorState testId="media-thumbnailError" error={data.thumbnailError ? String(data.thumbnailError) : null} />
      </Section>
      {!!thumb?.path && (
        <Facts
          title={t('media.thumbnailResult')}
          rows={[
            [t('media.width'), `${thumb.width ?? '--'} px`, 'media-thumbnailResult-width'],
            [t('media.height'), `${thumb.height ?? '--'} px`, 'media-thumbnailResult-height'],
            ['Type', thumb.type],
          ]}
        >
          <image
            data-testid="media-thumbnail-image"
            src={String(thumb.path)}
            alt={t('media.thumbnailAlt')}
            className="w-full rounded-lg border border-line bg-text/5 object-contain"
            style={{ height: 280 / Math.max(0.25, ratio) }}
          />
          <Path value={String(thumb.path)} />
          <Button testId="media-previewVideoThumbnail" onClick={() => actions.previewVideoThumbnail()}>
            {t('media.previewThumbnail')}
          </Button>
        </Facts>
      )}
      <Section title={t('media.compressVideo')} description={t('media.qualityLowMediumOr')}>
        <Inputs
          data={data}
          actions={actions}
          fields={[
            ['videoCompressQuality', t('media.quality'), 'onVideoCompressQualityInput', 'medium'],
            ['videoCompressBitrate', t('media.bitrateKbps'), 'onVideoCompressBitrateInput', '1200'],
            ['videoCompressFps', 'FPS', 'onVideoCompressFpsInput', '30'],
            ['videoCompressResolution', t('media.resolutionRatio'), 'onVideoCompressResolutionInput', '0.8'],
          ]}
        />
        <Button testId="media-compressSelectedVideo" disabled={!!data.videoCompressBusy} onClick={() => actions.compressSelectedVideo()}>
          {data.videoCompressBusy ? typeof data.videoCompressProgress === 'number' ? t('media.compressingPercent', { progress }) : t('media.compressing') : t('media.compressVideo')}
        </Button>
        {!!data.videoCompressBusy && (
          <view className="gap-3">
            <view data-testid="media-compression-progress" className="flex-row h-2 rounded-full bg-surface-sunken">
              <view className="h-2 rounded-full bg-success" style={{ width: `${progress}%` }} />
            </view>
            <Button variant="secondary" testId="media-cancelVideoCompress" onClick={() => actions.cancelVideoCompress()}>
              {t('media.cancel')}
            </Button>
          </view>
        )}
        <ErrorState testId="media-videoCompressError" error={data.videoCompressError ? String(data.videoCompressError) : null} />
      </Section>
      {!!result?.path && (
        <Facts
          title={t('media.compressedVideo')}
          rows={[
            [t('media.resolution'), dimensions(result), 'media-videoCompressResult-resolution'],
            [t('media.duration'), seconds(result.durationMs), 'media-videoCompressResult-durationMs'],
            ['Type', result.type],
            [t('media.fileSize'), fileSize(result.size), 'media-videoCompressResult-size'],
          ]}
        >
          <Path value={String(result.path)} testId="media-videoCompressResult-path" />
          <Button testId="media-previewCompressedVideo" onClick={() => actions.previewCompressedVideo()}>
            {t('media.previewCompressedVideo')}
          </Button>
        </Facts>
      )}
    </Page>
  );
}

type Picked = { path: string; type: string; displayWidth?: number; displayHeight?: number };

function Gallery({ data, actions }: Pane) {
  const video = data.mediaType === 'video';
  const items = (data.selectedMedia || []) as Picked[];
  const cameraOnly = data.sourceKey === 'camera';
  const limit = cameraOnly ? 1 : Number(data.countLimit ?? data.countKey ?? 9);
  const canAdd = !limit || items.length < limit;
  const source = ({ album: t('media.album'), camera: t('media.camera'), either: t('media.albumOrCamera') } as Record<string, string>)[String(data.sourceKey)] || t('media.album');
  const rotate = data.previewRotateKey && data.previewRotateKey !== 'meta' ? `${data.previewRotateKey}°` : 'Meta (Default)';
  const fit = data.previewObjectFitKey && data.previewObjectFitKey !== 'default' ? String(data.previewObjectFitKey) : t('media.fitDefault');
  const behavior =
    ({ manual: t('media.manual'), next: t('media.autoNext'), loop: t('media.loop') } as Record<string, string>)[String(data.previewBehaviorKey)] || t('media.manual');
  const result = data.previewSessionResult as { reason: string; index: number; source?: { path?: string } } | null;
  const option = (action: string, label: string, id = action) => (
    <Button variant="secondary" size="sm" testId={`media-${id}`} onClick={() => actions[action]()}>
      {label}
    </Button>
  );
  const add = (
    <button
      data-testid="media-launchMediaDemo"
      aria-label={said(String(data.addLabel))}
      disabled={!!data.isRunning || !canAdd}
      onClick={() => actions.launchMediaDemo()}
      className={
        video
          ? 'h-28 p-3 rounded-2xl border-2 border-accent/40 bg-accent-soft active:opacity-80 disabled:opacity-40'
          : 'h-24 p-3 rounded-2xl border-2 border-accent/40 bg-accent-soft active:opacity-80 disabled:opacity-40'
      }
    >
      <view className="items-center-safe justify-center gap-2">
        <text className="text-4xl text-accent">+</text>
        <text className="text-xs font-medium text-accent">{data.isRunning ? t('media.opening') : said(String(data.addLabel))}</text>
      </view>
    </button>
  );
  return (
    <Page intro={String(data.headerSubtitle || 'lx.pick.media / lx.media.preview')}>
      <Card className="divide-y divide-line">
        <Setting
          label={video ? t('media.videoSource') : t('media.photoSource')}
          value={source}
          testId="media-openSourcePicker"
          onClick={() => actions.openSourcePicker()}
        />
        <Setting
          label={t('media.countLimit')}
          value={String(limit)}
          testId="media-openCountPicker"
          disabled={cameraOnly}
          onClick={() => actions.openCountPicker()}
        />
        {video && (
          <Setting
            label={t('media.camera')}
            value={data.cameraKey === 'front' ? t('media.frontCamera') : t('media.rearCamera')}
            testId="media-openCameraPicker"
            onClick={() => actions.openCameraPicker()}
          />
        )}
        {video && (
          <Setting
            label={t('media.duration')}
            value={`${data.durationValue || 60} seconds`}
            testId="media-openDurationPicker"
            onClick={() => actions.openDurationPicker()}
          />
        )}
      </Card>
      <Section
        title={t('media.selection')}
        description={said(String(items.length ? data.previewHint : data.emptyHint))}
        action={
          limit > 0 ? (
            <text
              data-testid="media-selection-count"
              className="px-3 py-1 rounded-full bg-accent-soft text-xs font-semibold text-accent"
            >{`${items.length}/${limit}`}</text>
          ) : null
        }
      >
        <view className="grid gap-2 grid-cols-2">
          {option('openPreviewBehaviorPicker', t('media.previewFlow', { value: behavior }))}
          {option('openPreviewRotatePicker', t('media.previewRotate', { value: rotate }))}
          {option('openPreviewObjectFitPicker', t('media.previewFit', { value: fit }))}
          {option('togglePreviewIndexIndicator', t('media.indexIndicator', { value: data.previewHideIndexIndicator ? t('media.hidden') : t('media.auto') }), 'preview-index')}
        </view>
        <text className="text-[11px] text-muted">{t('media.nextAdvancesAndFinishes')}</text>
        {!video && (
          <view className="gap-1">
            <text className="text-xs font-medium text-muted">{t('media.imageDurationMs')}</text>
            <input
              data-testid="media-previewImageDurationMs"
              aria-label={t('media.imageDurationMs')}
              type="number"
              value={String(data.previewImageDurationMs ?? '2000')}
              onInput={value => actions.onPreviewImageDurationInput({ detail: { value } })}
              className="px-3 py-2 text-sm rounded-lg border border-line-strong bg-surface"
            />
          </view>
        )}
        <Button
          testId="media-previewSelectedMedia"
          disabled={!items.length || !!data.previewSessionBusy}
          onClick={() => actions.previewSelectedMedia()}
        >
          {t('media.previewSelection')}
        </Button>
        {!!data.previewSessionBusy && (
          <Button variant="destructive" size="sm" testId="media-cancelPreviewSession" onClick={() => actions.cancelPreviewSession()}>
            {t('media.cancelActivePreview')}
          </Button>
        )}
        {!!result && (
          <InfoList>
            <InfoRow label={t('media.reason')} value={result.reason} testId="media-preview-reason" />
            <InfoRow label={t('media.index')} value={result.index} testId="media-preview-index-result" />
            <InfoRow label={t('media.source')} value={result.source?.path} testId="media-preview-source" />
          </InfoList>
        )}
        <ErrorState testId="media-previewSessionError" error={data.previewSessionError ? String(data.previewSessionError) : null} />
        {!items.length ? (
          add
        ) : video ? (
          <view className="gap-3">
            {items.map((item, index) => (
              <Card key={`${item.path}-${index}`} className="px-4 py-3 gap-1">
                <text className="text-sm font-semibold">{`Video ${index + 1}`}</text>
                <text className="text-xs text-muted">{t('media.tapPreviewForFullscreen')}</text>
                {!!item.displayWidth && !!item.displayHeight && (
                  <text className="text-[11px] text-muted">{`${item.displayWidth} × ${item.displayHeight}`}</text>
                )}
              </Card>
            ))}
            {canAdd && add}
          </view>
        ) : (
          <view className="grid gap-3 grid-cols-2">
            {items.map((item, index) => (
              <Card key={`${item.path}-${index}`} className="overflow-hidden">
                <image
                  data-testid={`media-image-${index}`}
                  src={item.path}
                  alt={t('media.selectedImage', { n: index + 1 })}
                  className="h-32 w-full bg-surface-sunken object-cover"
                />
                <text className="px-3 py-3 text-xs font-medium">{`Image ${index + 1}`}</text>
              </Card>
            ))}
            {canAdd && add}
          </view>
        )}
      </Section>
    </Page>
  );
}

import { type Child, type View } from '@lingxia/ui';
import '../../theme/index.css';
import { Button, Card, ErrorState, InfoList, InfoRow, Page, Section } from '../../shared/ui';

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
      <row className="items-center gap-4">
        <text className="grow basis-0 text-sm font-medium text-muted">{label}</text>
        <text className="text-sm font-semibold">{value}</text>
        {!disabled && <text className="text-lg text-muted">›</text>}
      </row>
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
    <grid className="gap-3" style={{ gridTemplateColumns: columns }}>
      {fields.map(([key, label, action, placeholder]) => (
        <column key={key} className="gap-1.5">
          <text className="text-xs font-medium">{label}</text>
          <input
            data-testid={`media-${key}`}
            aria-label={label}
            value={String(data[key] ?? '')}
            placeholder={placeholder}
            onInput={value => actions[action]({ detail: { value } })}
            className="px-3 py-2.5 text-sm rounded-xl border border-line-strong bg-surface"
          />
        </column>
      ))}
    </grid>
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
      title={scan ? 'lx.scanCode' : 'Save to Album'}
      subtitle={scan ? 'Scan QR codes and barcodes using camera or album' : 'Capture photo or video and save to device album'}
    >
      {scan ? (
        <column className="gap-4">
          <Card className="divide-y divide-line">
            <Setting
              testId="media-openScanSourcePicker"
              label="Source"
              value={data.scanOnlyCamera ? 'Camera' : 'Camera & Album'}
              onClick={() => actions.openScanSourcePicker()}
            />
            <Setting
              testId="media-openScanTypePicker"
              label="Scan Type"
              value={String(data.scanTypeKey || 'all')}
              onClick={() => actions.openScanTypePicker()}
            />
          </Card>
          <Section title="Scan Result">
            <text
              data-testid="media-scan-result"
              className={
                data.scanResult
                  ? 'min-h-32 px-4 py-3 rounded-xl border border-line bg-surface-sunken font-mono break-all'
                  : 'min-h-32 px-4 py-3 rounded-xl border border-line bg-surface-sunken font-mono break-all text-muted'
              }
            >
              {String(data.scanResult || 'No result yet')}
            </text>
            <InfoRow label="Type" value={data.scanType} testId="media-scan-type" />
            <Button testId="media-startScan" disabled={!!data.scanBusy} onClick={() => actions.startScan()}>
              {data.scanBusy ? 'Scanning...' : 'Start Scan'}
            </Button>
          </Section>
        </column>
      ) : (
        <Section title="Capture" description="Capture photo or video, then save to album. Check your device album to view saved media.">
          <grid className="gap-3 grid-cols-2">
            <Button testId="media-captureImageForAlbum" disabled={!!data.saveToAlbumBusy} onClick={() => actions.captureImageForAlbum()}>
              {data.saveToAlbumBusy ? 'Saving...' : 'Capture Image'}
            </Button>
            <Button
              variant="secondary"
              testId="media-captureVideoForAlbum"
              disabled={!!data.saveToAlbumBusy}
              onClick={() => actions.captureVideoForAlbum()}
            >
              {data.saveToAlbumBusy ? 'Saving...' : 'Capture Video'}
            </Button>
          </grid>
        </Section>
      )}
    </Page>
  );
}

function ImageSummary({ title, info, prefix }: { title: string; info: Data; prefix: string }) {
  return (
    <Section title={title}>
      <InfoList>
        <InfoRow label="Width" value={info.width || 0} testId={`${prefix}-width`} />
        <InfoRow label="Height" value={info.height || 0} testId={`${prefix}-height`} />
        <InfoRow label="Type" value={info.type} testId={`${prefix}-type`} />
        <InfoRow label="File Size" value={fileSize(info.size)} testId={`${prefix}-size`} />
      </InfoList>
      {!!info.path && <Path value={info.path} testId={`${prefix}-path`} />}
    </Section>
  );
}

function ImageTools({ data, actions }: Pane) {
  const source = data.imageInfoResult as Data | null;
  const result = data.compressResult as Data | null;
  return (
    <Page title="Image Tools" subtitle="lx.getImageInfo and lx.compressImage: inspect an image and make a compressed copy">
      <Button testId="media-pickImageForInfo" disabled={!!data.imageInfoBusy} onClick={() => actions.pickImageForInfo()}>
        {data.imageInfoBusy ? 'Getting Info...' : 'Pick Image'}
      </Button>
      <ErrorState testId="media-imageInfoError" error={data.imageInfoError ? String(data.imageInfoError) : null} />
      {!!source && (
        <column className="gap-4">
          <ImageSummary title="Source Image" info={source} prefix="media-imageInfoResult" />
          <Section title="Compress">
            <Inputs
              data={data}
              actions={actions}
              columns={3}
              fields={[
                ['compressQuality', 'Quality', 'onCompressQualityInput', ''],
                ['compressedWidth', 'Width', 'onCompressedWidthInput', String(source.width || '')],
                ['compressedHeight', 'Height', 'onCompressedHeightInput', String(source.height || '')],
              ]}
            />
            <Button testId="media-compressSelectedImage" disabled={!!data.compressing} onClick={() => actions.compressSelectedImage()}>
              {data.compressing ? 'Compressing...' : 'Compress Image'}
            </Button>
            <ErrorState testId="media-compressError" error={data.compressError ? String(data.compressError) : null} />
          </Section>
        </column>
      )}
      {!!result && (
        <column className="gap-4">
          <ImageSummary title="Compressed Image" info={result} prefix="media-compressResult" />
          <Button variant="secondary" testId="media-previewCompressedImage" onClick={() => actions.previewCompressedImage()}>
            Preview Image
          </Button>
        </column>
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
    <Page title="Video Tools" subtitle="lx.getVideoInfo, lx.extractVideoThumbnail and lx.compressVideo">
      <Button testId="media-pickVideoForTools" disabled={!!data.thumbnailBusy || !!data.videoInfoBusy} onClick={() => actions.pickVideoForTools()}>
        {data.thumbnailVideoPath ? 'Pick Another Video' : 'Pick Video'}
      </Button>
      {!!data.thumbnailVideoPath && <Path value={String(data.thumbnailVideoPath)} />}
      <ErrorState testId="media-videoInfoError" error={data.videoInfoError ? String(data.videoInfoError) : null} />
      {!!info && (
        <Facts
          title="Upload Preflight Metadata"
          rows={[
            ['File Size', fileSize(info.size), 'media-videoInfoResult-size'],
            ['Resolution', dimensions(info), 'media-videoInfoResult-resolution'],
            ['Duration', seconds(info.durationMs), 'media-videoInfoResult-durationMs'],
            ['Rotation', info.rotation],
            ['Bitrate', info.bitrate > 0 ? `${Math.round(info.bitrate / 100) / 10} kbps` : '--'],
            ['FPS', info.fps],
            ['Container MIME', info.type || 'Not reported'],
            ['Video Codec', info.videoCodec || 'Not reported'],
            ['Has Audio', info.hasAudio == null ? 'Not reported' : info.hasAudio ? 'Yes' : 'No'],
            ['Audio Codec', info.hasAudio === false ? 'N/A' : info.audioCodec || 'Not reported'],
          ]}
        >
          <text className="text-[11px] text-muted">
            Track metadata is best-effort. The receiving service must validate uploaded bytes.
          </text>
          <Path value={String(info.path || '')} />
        </Facts>
      )}
      {!!source && (
        <Facts
          title="Source Video"
          rows={[
            ['Resolution', dimensions(source)],
            ['Duration', seconds(source.durationMs)],
            ['Type', source.type],
          ]}
        />
      )}
      <Section
        title="Thumbnail"
        description="Time 0 is the first frame. Empty sizes keep the source size; thumbnails scale proportionally and never crop."
      >
        <Inputs
          data={data}
          actions={actions}
          fields={[
            ['thumbnailQuality', 'Quality', 'onThumbnailQualityInput', '80'],
            ['thumbnailTimeMs', 'Time (ms)', 'onThumbnailTimeInput', '0'],
            ['thumbnailMaxWidth', 'Max Width (px)', 'onThumbnailMaxWidthInput', source?.width ? `e.g. ${source.width}` : 'leave empty'],
            ['thumbnailMaxHeight', 'Max Height (px)', 'onThumbnailMaxHeightInput', source?.height ? `e.g. ${source.height}` : 'leave empty'],
          ]}
        />
        <Button testId="media-createVideoThumbnail" disabled={!!data.thumbnailBusy} onClick={() => actions.createVideoThumbnail()}>
          {data.thumbnailBusy ? 'Generating...' : 'Generate Thumbnail'}
        </Button>
        <ErrorState testId="media-thumbnailError" error={data.thumbnailError ? String(data.thumbnailError) : null} />
      </Section>
      {!!thumb?.uri && (
        <Facts
          title="Thumbnail Result"
          rows={[
            ['Width', `${thumb.width ?? '--'} px`, 'media-thumbnailResult-width'],
            ['Height', `${thumb.height ?? '--'} px`, 'media-thumbnailResult-height'],
            ['Type', thumb.type],
          ]}
        >
          <image
            data-testid="media-thumbnail-image"
            src={String(thumb.uri)}
            alt="Thumbnail of the selected video"
            className="w-full rounded-lg border border-line bg-text/5 object-contain"
            style={{ height: 280 / Math.max(0.25, ratio) }}
          />
          <Path value={String(thumb.uri)} />
          <Button testId="media-previewVideoThumbnail" onClick={() => actions.previewVideoThumbnail()}>
            Preview Thumbnail
          </Button>
        </Facts>
      )}
      <Section title="Compress Video" description="Quality low, medium or high overrides bitrate (kbps), FPS and resolution ratio (0, 1].">
        <Inputs
          data={data}
          actions={actions}
          fields={[
            ['videoCompressQuality', 'Quality', 'onVideoCompressQualityInput', 'medium'],
            ['videoCompressBitrate', 'Bitrate (kbps)', 'onVideoCompressBitrateInput', '1200'],
            ['videoCompressFps', 'FPS', 'onVideoCompressFpsInput', '30'],
            ['videoCompressResolution', 'Resolution Ratio', 'onVideoCompressResolutionInput', '0.8'],
          ]}
        />
        <Button testId="media-compressSelectedVideo" disabled={!!data.videoCompressBusy} onClick={() => actions.compressSelectedVideo()}>
          {data.videoCompressBusy ? `Compressing...${typeof data.videoCompressProgress === 'number' ? ` ${progress}%` : ''}` : 'Compress Video'}
        </Button>
        {!!data.videoCompressBusy && (
          <column className="gap-3">
            <row data-testid="media-compression-progress" className="h-2 rounded-full bg-surface-sunken">
              <spacer className="h-2 rounded-full bg-success" style={{ width: `${progress}%` }} />
            </row>
            <Button variant="secondary" testId="media-cancelVideoCompress" onClick={() => actions.cancelVideoCompress()}>
              Cancel
            </Button>
          </column>
        )}
        <ErrorState testId="media-videoCompressError" error={data.videoCompressError ? String(data.videoCompressError) : null} />
      </Section>
      {!!result?.uri && (
        <Facts
          title="Compressed Video"
          rows={[
            ['Resolution', dimensions(result), 'media-videoCompressResult-resolution'],
            ['Duration', seconds(result.durationMs), 'media-videoCompressResult-durationMs'],
            ['Type', result.type],
            ['File Size', fileSize(result.size), 'media-videoCompressResult-size'],
          ]}
        >
          <Path value={String(result.uri)} testId="media-videoCompressResult-uri" />
          <Button testId="media-previewCompressedVideo" onClick={() => actions.previewCompressedVideo()}>
            Preview Compressed Video
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
  const source = ({ album: 'Album', camera: 'Camera', either: 'Album or Camera' } as Record<string, string>)[String(data.sourceKey)] || 'Album';
  const rotate = data.previewRotateKey && data.previewRotateKey !== 'meta' ? `${data.previewRotateKey}°` : 'Meta (Default)';
  const fit = data.previewObjectFitKey && data.previewObjectFitKey !== 'default' ? String(data.previewObjectFitKey) : 'Default (Optional)';
  const behavior =
    ({ manual: 'Manual Only', next: 'Auto Next', loop: 'Loop' } as Record<string, string>)[String(data.previewBehaviorKey)] || 'Manual Only';
  const result = data.previewSessionResult as { reason: string; index: number; source?: { path?: string } } | null;
  const option = (action: string, label: string, id = action) => (
    <Button variant="secondary" size="sm" testId={`media-${id}`} onClick={() => actions[action]()}>
      {label}
    </Button>
  );
  const add = (
    <button
      data-testid="media-launchMediaDemo"
      aria-label={String(data.addLabel)}
      disabled={!!data.isRunning || !canAdd}
      onClick={() => actions.launchMediaDemo()}
      className={
        video
          ? 'h-28 rounded-2xl border-2 border-accent/40 bg-accent-soft active:opacity-80 disabled:opacity-40'
          : 'h-24 rounded-2xl border-2 border-accent/40 bg-accent-soft active:opacity-80 disabled:opacity-40'
      }
    >
      <column className="items-center justify-center gap-2">
        <text className="text-4xl text-accent">+</text>
        <text className="text-xs font-medium text-accent">{data.isRunning ? 'Opening...' : String(data.addLabel)}</text>
      </column>
    </button>
  );
  return (
    <Page title="Media Manager" subtitle={String(data.headerSubtitle || 'lx.chooseMedia / lx.previewMedia')}>
      <Card className="divide-y divide-line">
        <Setting
          label={video ? 'Video Source' : 'Photo Source'}
          value={source}
          testId="media-openSourcePicker"
          onClick={() => actions.openSourcePicker()}
        />
        <Setting
          label="Count Limit"
          value={String(limit)}
          testId="media-openCountPicker"
          disabled={cameraOnly}
          onClick={() => actions.openCountPicker()}
        />
        {video && (
          <Setting
            label="Camera"
            value={data.cameraKey === 'front' ? 'Front Camera' : 'Rear Camera'}
            testId="media-openCameraPicker"
            onClick={() => actions.openCameraPicker()}
          />
        )}
        {video && (
          <Setting
            label="Duration"
            value={`${data.durationValue || 60} seconds`}
            testId="media-openDurationPicker"
            onClick={() => actions.openDurationPicker()}
          />
        )}
      </Card>
      <Section
        title="Selection"
        description={String(items.length ? data.previewHint : data.emptyHint)}
        action={
          limit > 0 ? (
            <text
              data-testid="media-selection-count"
              className="px-3 py-1 rounded-full bg-accent-soft text-xs font-semibold text-accent"
            >{`${items.length}/${limit}`}</text>
          ) : null
        }
      >
        <grid className="gap-2 grid-cols-2">
          {option('openPreviewBehaviorPicker', `Preview Flow: ${behavior}`)}
          {option('openPreviewRotatePicker', `Preview Rotate: ${rotate}`)}
          {option('openPreviewObjectFitPicker', `Preview Fit: ${fit}`)}
          {option('togglePreviewIndexIndicator', `Index Indicator: ${data.previewHideIndexIndicator ? 'Hidden' : 'Auto'}`, 'preview-index')}
        </grid>
        <text className="text-[11px] text-muted">`next` advances and finishes on the last item; `loop` wraps back to the first.</text>
        {!video && (
          <column className="gap-1">
            <text className="text-xs font-medium text-muted">Image Duration (ms)</text>
            <input
              data-testid="media-previewImageDurationMs"
              aria-label="Image Duration (ms)"
              type="number"
              value={String(data.previewImageDurationMs ?? '2000')}
              onInput={value => actions.onPreviewImageDurationInput({ detail: { value } })}
              className="px-3 py-2 text-sm rounded-lg border border-line-strong bg-surface"
            />
          </column>
        )}
        <Button
          testId="media-previewSelectedMedia"
          disabled={!items.length || !!data.previewSessionBusy}
          onClick={() => actions.previewSelectedMedia()}
        >
          Preview Selection
        </Button>
        {!!data.previewSessionBusy && (
          <Button variant="destructive" size="sm" testId="media-cancelPreviewSession" onClick={() => actions.cancelPreviewSession()}>
            Cancel Active Preview
          </Button>
        )}
        {!!result && (
          <InfoList>
            <InfoRow label="Reason" value={result.reason} testId="media-preview-reason" />
            <InfoRow label="Index" value={result.index} testId="media-preview-index-result" />
            <InfoRow label="Source" value={result.source?.path} testId="media-preview-source" />
          </InfoList>
        )}
        <ErrorState testId="media-previewSessionError" error={data.previewSessionError ? String(data.previewSessionError) : null} />
        {!items.length ? (
          add
        ) : video ? (
          <column className="gap-3">
            {items.map((item, index) => (
              <Card key={`${item.path}-${index}`} className="px-4 py-3 gap-1">
                <text className="text-sm font-semibold">{`Video ${index + 1}`}</text>
                <text className="text-xs text-muted">Tap Preview for fullscreen playback</text>
                {!!item.displayWidth && !!item.displayHeight && (
                  <text className="text-[11px] text-muted">{`${item.displayWidth} × ${item.displayHeight}`}</text>
                )}
              </Card>
            ))}
            {canAdd && add}
          </column>
        ) : (
          <grid className="gap-3 grid-cols-2">
            {items.map((item, index) => (
              <Card key={`${item.path}-${index}`} className="overflow-hidden">
                <image
                  data-testid={`media-image-${index}`}
                  src={item.path}
                  alt={`Selected image ${index + 1}`}
                  className="h-32 w-full bg-surface-sunken object-cover"
                />
                <text className="px-3 py-3 text-xs font-medium">{`Image ${index + 1}`}</text>
              </Card>
            ))}
            {canAdd && add}
          </grid>
        )}
      </Section>
    </Page>
  );
}

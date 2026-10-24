import { type View, type Props } from '@lingxia/ui';
import '../../theme/index.css';

export function render({ data, actions, env }: View) {
  if (data.mediaType === 'scanCode' || data.mediaType === 'saveToAlbum') return <Capture data={data} actions={actions} env={env} />;
  if (data.mediaType === 'imageInfo') return <ImageTools data={data} actions={actions} env={env} />;
  if (data.mediaType === 'videoTools') return <VideoTools data={data} actions={actions} env={env} />;
  return <MediaGallery data={data} actions={actions} env={env} />;
}

const mediaCard = 'bg-surface rounded-2xl shadow-sm border border-line-100';
const mediaPrimary = 'px-5 py-3 text-sm font-medium rounded-xl shadow-sm bg-linear-to-r from-blue-600 to-blue-500 text-white';
function MediaHeader({ title, subtitle, description }: { title: string; subtitle: string; description: string }) {
  return <column className={`${mediaCard} p-6 gap-3 items-center`}><text testId="media-page-title" className="text-center text-xl font-semibold text-gray-800">{title}</text><row className={subtitle.length > 40 ? "w-full items-center justify-center gap-2" : "items-center justify-center gap-2"}><spacer className="shrink-0 h-px w-8 bg-blue-400" /><text className={`text-center text-sm font-medium text-blue-600 dark:text-blue-400 ${subtitle.length > 40 ? "grow basis-0 w-0" : ""}`}>{subtitle}</text><spacer className="h-px w-8 bg-blue-400" /></row>{!!description && <text className="text-center text-sm text-gray-500 max-w-md leading-relaxed">{description}</text>}</column>;
}
function MediaSetting({ label, value, testId, onClick, enabled = true }: { label: string; value: string; testId: string; onClick: () => unknown; enabled?: boolean }) {
  return <button testId={testId} accessibilityLabel={`${label}: ${value}`} enabled={enabled} onClick={onClick} className="bg-surface px-6 py-4 text-sm"><row className="items-center gap-4"><text className="text-gray-600 font-medium shrink-0">{label}</text><spacer className="grow basis-0 h-px bg-line-200" /><text className="font-semibold text-gray-800">{value}</text>{enabled && <text className="text-gray-400 text-lg">{'\u203a'}</text>}</row></button>;
}
function Capture({ data: d, actions: a, env }: Pick<View, 'data' | 'actions' | 'env'>) {
  const scan = d.mediaType === 'scanCode';
  return <column className="bg-linear-to-br from-surface-50 to-surface-100"><spacer style={{ height: env.insets.top }} /><row className="grow basis-0"><spacer style={{ width: env.insets.left }} /><scroll testId="page-scroll" className="grow basis-0 px-4 pt-6 pb-16 gap-5">
    <MediaHeader title={scan ? 'lx.scanCode' : 'lx.saveImageToPhotosAlbum / lx.saveVideoToPhotosAlbum'} subtitle={scan ? 'QR & Barcode Scanner' : 'Save to Album'} description={scan ? 'Scan QR codes and barcodes using camera or album' : 'Capture photo or video and save to device album'} />
    {scan ? <column className="gap-5">
      <column className={mediaCard}><MediaSetting testId="media-openScanSourcePicker" label="Source" value={d.scanOnlyCamera ? 'Camera' : 'Camera & Album'} onClick={() => a.openScanSourcePicker()} /><spacer className="h-px bg-line-100" /><MediaSetting testId="media-openScanTypePicker" label="Scan Type" value={String(d.scanTypeKey || 'all')} onClick={() => a.openScanTypePicker()} /></column>
      <column className={`${mediaCard} p-6 gap-4`}><column className="gap-2"><row className="items-center gap-2"><spacer className="w-1 h-4 bg-blue-500 rounded-full" /><text className="text-sm font-semibold text-gray-700">Scan Result</text></row>
        <text testId="media-scan-result" className={`min-h-32 rounded-xl bg-linear-to-br from-surface-50 to-surface-100 px-5 py-4 text-base border border-line-200 font-mono break-all ${d.scanResult ? 'text-gray-900' : 'text-gray-400'}`}>{String(d.scanResult || 'No result yet')}</text>
        <row className="items-center gap-2"><text className="text-xs text-gray-500 font-medium">Type:</text><text testId="media-scan-type" className="text-xs text-gray-500 px-2 py-1 bg-surface-100 rounded-md">{String(d.scanType || '--')}</text></row>
      </column><button testId="media-startScan" enabled={!d.scanBusy} onClick={() => a.startScan()} className={mediaPrimary}>{d.scanBusy ? 'Scanning...' : 'Start Scan'}</button></column>
    </column> : <column className={`${mediaCard} p-6 gap-5`}>
      <column className="bg-blue-50 px-4 py-3 rounded-xl border border-blue-100"><text className="text-sm text-gray-600">{'\u{1f4f8} Capture photo or video, then save to album. Check your device album to view saved media.'}</text></column>
      <grid columns={2} className="gap-4"><button testId="media-captureImageForAlbum" enabled={!d.saveToAlbumBusy} onClick={() => a.captureImageForAlbum()} className={mediaPrimary}>{d.saveToAlbumBusy ? 'Saving...' : 'Capture Image'}</button><button testId="media-captureVideoForAlbum" enabled={!d.saveToAlbumBusy} onClick={() => a.captureVideoForAlbum()} className="px-5 py-3 text-sm font-medium rounded-xl shadow-sm bg-linear-to-r from-green-600 to-green-500 text-white">{d.saveToAlbumBusy ? 'Saving...' : 'Capture Video'}</button></grid>
    </column>}
  </scroll><spacer style={{ width: env.insets.right }} /></row><spacer style={{ height: Math.max(env.insets.bottom, env.keyboard.height) }} /></column>;
}


type ImageInfo = { width?: number; height?: number; type?: string; size?: number; path?: string };
function mediaFileSize(value: number = 0) {
  if (!value) return '0 B';
  const i = Math.min(3, Math.max(0, Math.floor(Math.log(value) / Math.log(1024))));
  return `${(value / Math.pow(1024, i)).toFixed(2)} ${['B', 'KB', 'MB', 'GB'][i]}`;
}
function ImageInfoCard({ title, info, prefix }: { title: string; info: ImageInfo; prefix: string }) {
  return <column className="rounded-xl border border-line-200 bg-linear-to-br from-surface-50 to-surface p-5 gap-4">
    <row className="items-center gap-2"><spacer className="w-1 h-4 bg-blue-500 rounded-full" /><text className="text-sm font-semibold text-gray-700">{title}</text></row>
    <column className="gap-3"><row className="items-center justify-between gap-3"><text className="text-sm text-gray-600">Dimensions</text><row className="gap-1"><text testId={`${prefix}-width`} className="text-sm font-semibold text-gray-800">{String(info.width || 0)}</text><text className="text-sm font-semibold text-gray-800">{'\u00d7'}</text><text testId={`${prefix}-height`} className="text-sm font-semibold text-gray-800">{String(info.height || 0)}</text></row></row>
    <row className="items-center justify-between gap-3"><text className="text-sm text-gray-600">Type</text><text testId={`${prefix}-type`} className="text-sm font-semibold text-gray-800">{info.type || '--'}</text></row>
    <row className="items-center justify-between gap-3"><text className="text-sm text-gray-600">File Size</text><text testId={`${prefix}-size`} className="text-sm font-semibold text-gray-800">{mediaFileSize(info.size)}</text></row></column>
    {!!info.path && <column className="pt-4 border-t border-line-200 gap-1"><text className="text-xs font-medium text-gray-700">Path</text><column className="bg-surface-100 px-3 py-2 rounded-lg"><text testId={`${prefix}-path`} className="text-[11px] text-gray-500 break-all">{info.path}</text></column></column>}
  </column>;
}
function MediaError({ value, testId }: { value: unknown; testId: string }) {
  return <row className="items-center gap-2 bg-red-50 px-4 py-3 rounded-xl"><text className="text-sm text-red-600 dark:text-red-400">{'\u26a0'}</text><text testId={testId} className="grow basis-0 min-w-0 text-sm text-red-600 dark:text-red-400">{String(value)}</text></row>;
}
function ImageTools({ data: d, actions: a, env }: Pick<View, 'data' | 'actions' | 'env'>) {
  const source = d.imageInfoResult as ImageInfo | null;
  const result = d.compressResult as ImageInfo | null;
  return <column className="bg-linear-to-br from-surface-50 to-surface-100"><spacer style={{ height: env.insets.top }} /><row className="grow basis-0"><spacer style={{ width: env.insets.left }} /><scroll testId="page-scroll" className="grow basis-0 px-4 pt-6 pb-16 gap-5">
    <MediaHeader title="lx.getImageInfo / lx.compressImage" subtitle="Image Tools" description="Get image info and create compressed copy" />
    <column className={`${mediaCard} p-6 gap-5`}>
      <button testId="media-pickImageForInfo" enabled={!d.imageInfoBusy} onClick={() => a.pickImageForInfo()} className={mediaPrimary}>{d.imageInfoBusy ? 'Getting Info...' : 'Pick Image'}</button>{!!d.imageInfoError && <MediaError value={d.imageInfoError} testId="media-imageInfoError" />}
      {!!source && <column className="gap-5"><ImageInfoCard title="Source Image" info={source} prefix="media-imageInfoResult" />
        <grid columns={3} className="gap-3">{[['compressQuality','Quality','onCompressQualityInput',''],['compressedWidth','Width','onCompressedWidthInput',String(source.width || '')],['compressedHeight','Height','onCompressedHeightInput',String(source.height || '')]].map(([key,label,action,placeholder]) => <column key={key} className="gap-2"><text className="text-xs font-medium text-gray-700">{label}</text><input testId={`media-${key}`} value={String(d[key] ?? '')} placeholder={placeholder} onInput={value => a[action]({ detail: { value } })} className="px-4 py-2.5 text-sm text-gray-900 border border-line-200 rounded-xl bg-surface" /></column>)}</grid>
        <button testId="media-compressSelectedImage" enabled={!d.compressing} onClick={() => a.compressSelectedImage()} className={mediaPrimary}>{d.compressing ? 'Compressing...' : 'Compress Image'}</button>{!!d.compressError && <MediaError value={d.compressError} testId="media-compressError" />}
        {!!result && <column className="gap-4"><ImageInfoCard title="Compressed Image" info={result} prefix="media-compressResult" /><button testId="media-previewCompressedImage" onClick={() => a.previewCompressedImage()} className="px-5 py-3 text-sm font-medium rounded-xl shadow-sm bg-linear-to-r from-surface-600 to-surface-500 text-white">Preview Image</button></column>}
      </column>}
    </column>
  </scroll><spacer style={{ width: env.insets.right }} /></row><spacer style={{ height: Math.max(env.insets.bottom, env.keyboard.height) }} /></column>;
}

function MediaInfo({ title, rows, children }: Pick<Props, 'children'> & { title: string; rows: { label: string; value: string; id?: string }[] }) {
  return <column className="rounded-xl border border-line-200 bg-linear-to-br from-surface-50 to-surface p-5 gap-4"><row className="items-center gap-2"><spacer className="w-1 h-4 bg-blue-500 rounded-full" /><text className="text-sm font-semibold text-gray-700">{title}</text></row><column className="gap-3">{rows.map(row => <row key={row.label} className="items-center justify-between gap-3"><text className="shrink-0 text-sm text-gray-600">{row.label}</text><text testId={row.id} className="max-w-[72%] break-all text-right text-sm font-semibold text-gray-800">{row.value}</text></row>)}</column>{children}</column>;
}
function MediaPath({ value, testId }: { value: string; testId?: string }) {
  return <column className="bg-surface-100 px-3 py-2 rounded-lg"><text testId={testId} className="text-[11px] text-gray-500 break-all">{value}</text></column>;
}
function VideoTools({ data: d, actions: a, env }: Pick<View, 'data' | 'actions' | 'env'>) {
  const info = d.videoInfoResult as Record<string, any> | null;
  const source = d.thumbnailSourceInfo as Record<string, any> | null;
  const thumb = d.thumbnailResult as Record<string, any> | null;
  const result = d.videoCompressResult as Record<string, any> | null;
  const duration = (value: number) => value > 0 ? `${(value / 1000).toFixed(2)} s` : '--';
  const dimensions = (v: Record<string, any>) => `${v.width ?? '--'} \u00d7 ${v.height ?? '--'}`;
  const row = (label: string, value: unknown, id?: string) => ({ label, value: String(value ?? '--'), id });
  const fields = (values: string[][]) => <grid columns={2} className="gap-3">{values.map(([key,label,action,placeholder]) => <column key={key} className="gap-2"><text className="text-xs font-medium text-gray-700">{label}</text><input testId={`media-${key}`} value={String(d[key] ?? '')} placeholder={placeholder} onInput={value => a[action]({ detail: { value } })} className="px-4 py-2.5 text-sm text-gray-900 border border-line-200 rounded-xl bg-surface" /></column>)}</grid>;
  const progress = Math.max(0, Math.min(100, Number(d.videoCompressProgress) || 0));
  const thumbWidth = Math.max(1, env.viewport.width - env.insets.left - env.insets.right - 124);
  return <column className="bg-linear-to-br from-surface-50 to-surface-100"><spacer style={{ height: env.insets.top }} /><row className="grow basis-0"><spacer style={{ width: env.insets.left }} /><scroll testId="page-scroll" className="grow basis-0 px-4 pt-6 pb-16 gap-5">
    <MediaHeader title="Video Tools" subtitle="lx.getVideoInfo / lx.extractVideoThumbnail / lx.compressVideo" description="Get video info, generate thumbnail, and create compressed copy" />
    <column className={`${mediaCard} p-6 gap-5`}>
      <button testId="media-pickVideoForTools" enabled={!d.thumbnailBusy && !d.videoInfoBusy} onClick={() => a.pickVideoForTools()} className={mediaPrimary}>{d.thumbnailVideoPath ? 'Pick Another Video' : 'Pick Video'}</button>
      {!!d.thumbnailVideoPath && <column className="rounded-xl border border-line-200 bg-surface-50 px-4 py-3"><text className="text-xs text-gray-600 break-all">{String(d.thumbnailVideoPath)}</text></column>}
      {!!d.videoInfoError && <MediaError value={d.videoInfoError} testId="media-videoInfoError" />}
      {!!info && <MediaInfo title="Upload Preflight Metadata" rows={[
        row('File Size',mediaFileSize(info.size),'media-videoInfoResult-size'),row('Resolution',dimensions(info),'media-videoInfoResult-resolution'),row('Duration',duration(info.durationMs),'media-videoInfoResult-durationMs'),row('Rotation',info.rotation),row('Bitrate',info.bitrate > 0 ? `${Math.round(info.bitrate / 100) / 10} kbps` : '--'),row('FPS',info.fps),row('Container MIME',info.type || 'Not reported'),row('Video Codec',info.videoCodec || 'Not reported'),row('Has Audio',info.hasAudio == null ? 'Not reported' : info.hasAudio ? 'Yes' : 'No'),row('Audio Codec',info.hasAudio === false ? 'N/A' : info.audioCodec || 'Not reported')
      ]}><column className="pt-4 border-t border-line-200 gap-3"><text className="text-[11px] text-gray-500">Track metadata is best-effort. The receiving service must validate uploaded bytes.</text><column className="gap-1"><text className="text-xs font-medium text-gray-700">Path</text><MediaPath value={String(info.path || '')} /></column></column></MediaInfo>}
      {!!source && <MediaInfo title="Source Video" rows={[row('Resolution',dimensions(source)),row('Duration',duration(source.durationMs)),row('Type',source.type || '--')]} />}
      <column className="bg-blue-50 border border-blue-100 rounded-xl px-3 py-2"><text className="text-xs text-gray-600">{'Quality: 0-100\nTime (ms): 0 means first frame\nMax Width / Max Height unit: px\nLeave Max Width/Height empty to keep original size\nThumbnail is scaled proportionally, not cropped\nIf Max Width/Height exceeds source, it is clamped automatically'}</text></column>
      {fields([['thumbnailQuality','Quality','onThumbnailQualityInput','80'],['thumbnailTimeMs','Time (ms)','onThumbnailTimeInput','0'],['thumbnailMaxWidth','Max Width (px)','onThumbnailMaxWidthInput',source?.width ? `e.g. ${source.width}` : 'leave empty'],['thumbnailMaxHeight','Max Height (px)','onThumbnailMaxHeightInput',source?.height ? `e.g. ${source.height}` : 'leave empty']])}
      <button testId="media-createVideoThumbnail" enabled={!d.thumbnailBusy} onClick={() => a.createVideoThumbnail()} className={mediaPrimary}>{d.thumbnailBusy ? 'Generating...' : 'Generate Thumbnail'}</button>
      {!!d.thumbnailError && <MediaError value={d.thumbnailError} testId="media-thumbnailError" />}
      {!!thumb?.uri && <column className="gap-4"><MediaInfo title="Thumbnail Result" rows={[row('Width',`${thumb.width ?? '--'} px`,'media-thumbnailResult-width'),row('Height',`${thumb.height ?? '--'} px`,'media-thumbnailResult-height'),row('Type',thumb.type || '--')]}><column className="pt-4 border-t border-line-200 gap-2"><image testId="media-thumbnail-image" src={String(thumb.uri)} alt="Thumbnail of the selected video" fit="contain" className="w-full rounded-lg border border-line-200 bg-black/5" style={{ height: thumbWidth * Number(thumb.height || 1) / Math.max(1,Number(thumb.width || 1)) }} /><MediaPath value={String(thumb.uri)} /></column></MediaInfo><button testId="media-previewVideoThumbnail" onClick={() => a.previewVideoThumbnail()} className={mediaPrimary}>Preview Thumbnail</button></column>}
      <column className="rounded-xl border border-line-200 bg-linear-to-br from-surface-50 to-surface p-5 gap-4"><row className="items-center gap-2"><spacer className="w-1 h-4 bg-indigo-500 rounded-full" /><text className="text-sm font-semibold text-gray-700">Compress Video</text></row>
        <column className="bg-indigo-50 border border-indigo-100 rounded-lg px-3 py-2"><text className="text-xs text-gray-600">{'Quality: low / medium / high\nIf quality is set, bitrate/fps/resolution are ignored.\nBitrate unit: kbps, FPS unit: frame/s, Resolution range: (0, 1]'}</text></column>
        {fields([['videoCompressQuality','Quality','onVideoCompressQualityInput','medium'],['videoCompressBitrate','Bitrate (kbps)','onVideoCompressBitrateInput','1200'],['videoCompressFps','FPS','onVideoCompressFpsInput','30'],['videoCompressResolution','Resolution Ratio','onVideoCompressResolutionInput','0.8']])}
        <button testId="media-compressSelectedVideo" enabled={!d.videoCompressBusy} onClick={() => a.compressSelectedVideo()} className="px-5 py-3 text-sm font-medium rounded-xl shadow-sm bg-linear-to-r from-green-600 to-green-500 text-white">{d.videoCompressBusy ? `Compressing...${typeof d.videoCompressProgress === 'number' ? ` ${progress}%` : ''}` : 'Compress Video'}</button>
        {!!d.videoCompressBusy && <column className="gap-4"><row testId="media-compression-progress" className="h-2 rounded-full bg-surface-200"><spacer className="h-2 rounded-full bg-green-500" style={{ width: `${progress}%` }} /></row><button testId="media-cancelVideoCompress" onClick={() => a.cancelVideoCompress()} className="px-5 py-3 text-sm font-medium rounded-xl shadow-sm bg-linear-to-r from-surface-600 to-surface-500 text-white">Cancel</button></column>}
        {!!d.videoCompressError && <MediaError value={d.videoCompressError} testId="media-videoCompressError" />}
        {!!result?.uri && <column className="gap-3"><MediaInfo title="Compressed Video" rows={[row('Resolution',dimensions(result),'media-videoCompressResult-resolution'),row('Duration',duration(result.durationMs),'media-videoCompressResult-durationMs'),row('Type',result.type || '--'),row('File Size',mediaFileSize(result.size),'media-videoCompressResult-size')]}><column className="pt-4 border-t border-line-200"><MediaPath value={String(result.uri)} testId="media-videoCompressResult-uri" /></column></MediaInfo><button testId="media-previewCompressedVideo" onClick={() => a.previewCompressedVideo()} className={mediaPrimary}>Preview Compressed Video</button></column>}
      </column>
    </column>
  </scroll><spacer style={{ width: env.insets.right }} /></row><spacer style={{ height: Math.max(env.insets.bottom, env.keyboard.height) }} /></column>;
}


function MediaGallery({ data: d, actions: a, env }: Pick<View, 'data' | 'actions' | 'env'>) {
  const video = d.mediaType === 'video';
  const items = (d.selectedMedia || []) as { path: string; type: string; displayWidth?: number; displayHeight?: number }[];
  const cameraOnly = d.sourceKey === 'camera';
  const limit = cameraOnly ? 1 : Number(d.countLimit ?? d.countKey ?? 9);
  const canAdd = !limit || items.length < limit;
  const source = ({ album: 'Album', camera: 'Camera', either: 'Album or Camera' } as Record<string, string>)[String(d.sourceKey)] || 'Album';
  const rotate = d.previewRotateKey && d.previewRotateKey !== 'meta' ? `${d.previewRotateKey}\u00b0` : 'Meta (Default)';
  const fit = d.previewObjectFitKey && d.previewObjectFitKey !== 'default' ? String(d.previewObjectFitKey) : 'Default (Optional)';
  const behavior = ({ manual: 'Manual Only', next: 'Auto Next', loop: 'Loop' } as Record<string, string>)[String(d.previewBehaviorKey)] || 'Manual Only';
  const control = (action: string, label: string, id = action) => <button testId={`media-${id}`} accessibilityLabel={label} onClick={() => a[action]()} className="rounded-xl border border-line-200 bg-surface px-3 py-2"><text className="text-left text-xs font-medium text-gray-700">{label}</text></button>;
  const add = () => <button testId="media-launchMediaDemo" accessibilityLabel={String(d.addLabel)} enabled={!d.isRunning && canAdd} onClick={() => a.launchMediaDemo()} className={`w-full ${video ? 'h-28' : 'h-24'} rounded-2xl border-2 ${d.isRunning || !canAdd ? 'border-line-200 bg-surface-50 opacity-40' : 'border-blue-300 bg-linear-to-br from-blue-50 to-indigo-50'}`}><column className="items-center justify-center gap-2"><text className="text-4xl text-blue-500 dark:text-blue-400">+</text><text className="text-xs font-medium text-blue-600 dark:text-blue-400">{d.isRunning ? 'Opening...' : String(d.addLabel)}</text></column></button>;
  const result = d.previewSessionResult as { reason: string; index: number; source?: { path?: string } } | null;
  return <column className="bg-linear-to-br from-surface-50 to-surface-100"><spacer style={{ height: env.insets.top }} /><row className="grow basis-0"><spacer style={{ width: env.insets.left }} /><scroll testId="page-scroll" className="grow basis-0 px-4 pt-6 pb-16 gap-5">
    <MediaHeader title="Media Manager" subtitle={String(d.headerSubtitle || 'lx.chooseMedia / lx.previewMedia')} description="" />
    <column className={mediaCard}>
      <MediaSetting label={video ? 'Video Source' : 'Photo Source'} value={source} testId="media-openSourcePicker" onClick={() => a.openSourcePicker()} />
      <spacer className="h-px bg-line-100" /><MediaSetting label="Count Limit" value={String(limit)} testId="media-openCountPicker" enabled={!cameraOnly} onClick={() => a.openCountPicker()} />
      {video && <column><spacer className="h-px bg-line-100" /><MediaSetting label="Camera" value={d.cameraKey === 'front' ? 'Front Camera' : 'Rear Camera'} testId="media-openCameraPicker" onClick={() => a.openCameraPicker()} /><spacer className="h-px bg-line-100" /><MediaSetting label="Duration" value={`${d.durationValue || 60} seconds`} testId="media-openDurationPicker" onClick={() => a.openDurationPicker()} /></column>}
    </column>
    <column className={`${mediaCard} p-6 gap-4`}>
      <row className="items-center gap-3"><text className="grow basis-0 w-0 text-sm text-gray-600">{String(items.length ? d.previewHint : d.emptyHint)}</text>{limit > 0 && <text testId="media-selection-count" className="px-3 py-1 bg-blue-50 text-blue-600 dark:text-blue-400 text-xs font-semibold rounded-full">{`${items.length}/${limit}`}</text>}</row>
      <grid columns={2} className="gap-2">
        {control('openPreviewBehaviorPicker', `Preview Flow: ${behavior}`)}
        {control('openPreviewRotatePicker', `Preview Rotate: ${rotate}`)}
        {control('openPreviewObjectFitPicker', `Preview Fit: ${fit}`)}
        {control('togglePreviewIndexIndicator', `Index Indicator: ${d.previewHideIndexIndicator ? 'Hidden' : 'Auto'}`, 'preview-index')}
      </grid>
      <text className="text-[11px] text-gray-500">{'`next` advances to the next item and finishes on the last item. `loop` wraps back to the first item.'}</text>
      {!video && <column className="gap-1"><text className="text-xs font-medium text-gray-600">Image Duration (ms)</text><input testId="media-previewImageDurationMs" keyboard="number" value={String(d.previewImageDurationMs ?? '2000')} onInput={value => a.onPreviewImageDurationInput({ detail: { value } })} className="w-full rounded-lg border border-line-200 bg-surface px-3 py-2 text-sm text-gray-800" /></column>}
      <button testId="media-previewSelectedMedia" enabled={items.length > 0 && !d.previewSessionBusy} onClick={() => a.previewSelectedMedia()} className={mediaPrimary}>Preview Selection</button>
      <text className="text-[11px] text-gray-500">Preview Rotate/Fit applies to both images and videos.</text>
      {!!d.previewSessionBusy && <button testId="media-cancelPreviewSession" onClick={() => a.cancelPreviewSession()} className="px-4 py-2 text-xs font-medium rounded-xl bg-linear-to-r from-red-600 to-red-500 text-white">Cancel Active Preview</button>}
      {!!result && <MediaInfo title="Last Preview Result" rows={[{label:'Reason',value:String(result.reason),id:'media-preview-reason'},{label:'Index',value:String(result.index),id:'media-preview-index-result'},{label:'Source',value:String(result.source?.path || ''),id:'media-preview-source'}]} />}
      {!!d.previewSessionError && <MediaError value={d.previewSessionError} testId="media-previewSessionError" />}
      {items.length === 0 ? add() : video ? <column className="gap-4">{items.map((item,index) => <column key={`${item.path}-${index}`} className={`${mediaCard} px-5 py-4 gap-2`}><text className="text-sm font-semibold text-gray-800">{`Video ${index+1}`}</text><text className="text-xs text-gray-500">Tap Preview for fullscreen playback</text>{!!item.displayWidth && !!item.displayHeight && <text className="text-[11px] text-gray-500">{`${item.displayWidth} \u00d7 ${item.displayHeight}`}</text>}</column>)}{canAdd && add()}</column> : <grid columns={2} className="gap-3">{items.map((item,index) => <column key={`${item.path}-${index}`} className={mediaCard}><image testId={`media-image-${index}`} src={item.path} alt={`Selected image ${index+1}`} fit="cover" className="h-32 w-full bg-surface-50" /><column className="px-3 py-3 bg-linear-to-br from-surface-50 to-surface"><text className="text-xs font-medium text-gray-700">{`Image ${index+1}`}</text></column></column>)}{canAdd && add()}</grid>}
    </column>
  </scroll><spacer style={{ width: env.insets.right }} /></row><spacer style={{ height: Math.max(env.insets.bottom, env.keyboard.height) }} /></column>;
}

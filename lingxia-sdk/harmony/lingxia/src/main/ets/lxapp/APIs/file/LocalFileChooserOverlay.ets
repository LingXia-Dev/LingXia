import { hilog } from '@kit.PerformanceAnalysisKit';
import { fileIo } from '@kit.CoreFileKit';
import { onCallback } from 'liblingxia.so';

const DOMAIN = 0x0000;
const TAG = 'LingXia.LocalFileChooser';

export type LocalChooserMode = 'file' | 'directory';

export interface LocalFileChooserOverlayState {
  title: string;
  rootPath: string;
  mode: LocalChooserMode;
  callbackId: string;
  filters: string[];
}

interface LocalFilterSpec {
  extensions: Set<string>;
  exactMimeTypes: Set<string>;
  wildcardMimeGroups: Set<string>;
  labels: string[];
}

interface LocalFileChooserEntry {
  path: string;
  name: string;
  isDirectory: boolean;
  badge: string;
  subtitle: string;
}

class LocalDirectoryListingResult {
  entries: LocalFileChooserEntry[] = [];
  hasFilteredOutFiles: boolean = false;
}

const MIME_BY_EXTENSION: Map<string, string> = new Map<string, string>([
  ['jpg', 'image/jpeg'],
  ['jpeg', 'image/jpeg'],
  ['png', 'image/png'],
  ['gif', 'image/gif'],
  ['webp', 'image/webp'],
  ['bmp', 'image/bmp'],
  ['heic', 'image/heic'],
  ['heif', 'image/heif'],
  ['svg', 'image/svg+xml'],
  ['mp4', 'video/mp4'],
  ['m4v', 'video/mp4'],
  ['mov', 'video/quicktime'],
  ['mkv', 'video/x-matroska'],
  ['webm', 'video/webm'],
  ['avi', 'video/x-msvideo'],
  ['mp3', 'audio/mpeg'],
  ['wav', 'audio/wav'],
  ['m4a', 'audio/mp4'],
  ['aac', 'audio/aac'],
  ['flac', 'audio/flac'],
  ['ogg', 'audio/ogg'],
  ['pdf', 'application/pdf'],
  ['txt', 'text/plain'],
  ['csv', 'text/csv'],
  ['json', 'application/json'],
  ['doc', 'application/msword'],
  ['docx', 'application/vnd.openxmlformats-officedocument.wordprocessingml.document'],
  ['xls', 'application/vnd.ms-excel'],
  ['xlsx', 'application/vnd.openxmlformats-officedocument.spreadsheetml.sheet'],
  ['ppt', 'application/vnd.ms-powerpoint'],
  ['pptx', 'application/vnd.openxmlformats-officedocument.presentationml.presentation'],
]);

export class LocalFileChooserOverlayManager {
  private static state: LocalFileChooserOverlayState | null = null;
  private static listeners: Set<() => void> = new Set();

  public static show(state: LocalFileChooserOverlayState): boolean {
    if (!state.rootPath) {
      return false;
    }
    LocalFileChooserOverlayManager.state = state;
    LocalFileChooserOverlayManager.notifyListeners();
    return true;
  }

  public static hide(): void {
    LocalFileChooserOverlayManager.state = null;
    LocalFileChooserOverlayManager.notifyListeners();
  }

  public static isShowing(): boolean {
    return LocalFileChooserOverlayManager.state !== null;
  }

  public static getState(): LocalFileChooserOverlayState | null {
    return LocalFileChooserOverlayManager.state;
  }

  public static cancel(): void {
    const callbackId = String(LocalFileChooserOverlayManager.state?.callbackId ?? '').trim();
    if (callbackId) {
      onCallback(callbackId, true, JSON.stringify({ canceled: true, paths: [] }));
    }
    LocalFileChooserOverlayManager.hide();
  }

  public static complete(paths: string[]): void {
    const callbackId = String(LocalFileChooserOverlayManager.state?.callbackId ?? '').trim();
    if (callbackId) {
      onCallback(callbackId, true, JSON.stringify({
        canceled: !paths || paths.length === 0,
        paths: paths ?? []
      }));
    }
    LocalFileChooserOverlayManager.hide();
  }

  public static registerListener(listener: () => void): void {
    LocalFileChooserOverlayManager.listeners.add(listener);
  }

  public static unregisterListener(listener: () => void): void {
    LocalFileChooserOverlayManager.listeners.delete(listener);
  }

  private static notifyListeners(): void {
    LocalFileChooserOverlayManager.listeners.forEach((listener: () => void) => {
      try {
        listener();
      } catch (error) {
        hilog.error(DOMAIN, TAG, `listener error: ${error}`);
      }
    });
  }
}

function trimSlashes(input: string): string {
  return String(input ?? '').replace(/\/+$/, '');
}

function parentDirectory(path: string): string {
  const normalized = trimSlashes(path);
  const index = normalized.lastIndexOf('/');
  if (index <= 0) {
    return normalized;
  }
  return normalized.substring(0, index);
}

function fileName(path: string): string {
  const normalized = trimSlashes(path);
  const index = normalized.lastIndexOf('/');
  return index >= 0 ? normalized.substring(index + 1) : normalized;
}

function isWithinRoot(candidate: string, rootPath: string): boolean {
  const normalizedRoot = trimSlashes(rootPath);
  const normalizedCandidate = trimSlashes(candidate);
  return normalizedCandidate === normalizedRoot || normalizedCandidate.startsWith(`${normalizedRoot}/`);
}

function canListDirectory(path: string): boolean {
  try {
    fileIo.listFileSync(path);
    return true;
  } catch (_) {
    return false;
  }
}

function resolveChooserRoot(defaultPath: string): string | null {
  const trimmed = trimSlashes(defaultPath).trim();
  if (!trimmed) {
    return null;
  }
  if (canListDirectory(trimmed)) {
    return trimmed;
  }
  const parent = parentDirectory(trimmed);
  return parent && canListDirectory(parent) ? parent : null;
}

function formatFileSize(size: number): string {
  if (!Number.isFinite(size) || size <= 0) {
    return '0 B';
  }
  const units: string[] = ['B', 'KB', 'MB', 'GB'];
  let value: number = size;
  let unitIndex: number = 0;
  while (value >= 1024 && unitIndex < units.length - 1) {
    value /= 1024;
    unitIndex += 1;
  }
  const digits: number = unitIndex === 0 ? 0 : (value >= 10 ? 1 : 2);
  return `${value.toFixed(digits)} ${units[unitIndex]}`;
}

function relativeTimeLabel(timestampMs: number): string {
  if (!Number.isFinite(timestampMs) || timestampMs <= 0) {
    return '';
  }
  const diff = Date.now() - timestampMs;
  const abs = Math.abs(diff);
  const minute = 60 * 1000;
  const hour = 60 * minute;
  const day = 24 * hour;
  if (abs < hour) {
    return `${Math.max(1, Math.round(abs / minute))}m`;
  }
  if (abs < day) {
    return `${Math.max(1, Math.round(abs / hour))}h`;
  }
  return `${Math.max(1, Math.round(abs / day))}d`;
}

function isFilterSpecEmpty(filterSpec: LocalFilterSpec): boolean {
  return filterSpec.extensions.size === 0 &&
    filterSpec.exactMimeTypes.size === 0 &&
    filterSpec.wildcardMimeGroups.size === 0;
}

function guessMimeType(name: string): string {
  const extIndex = name.lastIndexOf('.');
  const ext = extIndex >= 0 ? name.substring(extIndex + 1).toLowerCase() : '';
  return ext.length > 0 ? String(MIME_BY_EXTENSION.get(ext) ?? '') : '';
}

function matchesFileFilter(name: string, filterSpec: LocalFilterSpec): boolean {
  if (isFilterSpecEmpty(filterSpec)) {
    return true;
  }

  const extIndex = name.lastIndexOf('.');
  const ext = extIndex >= 0 ? name.substring(extIndex + 1).toLowerCase() : '';
  if (ext.length > 0 && filterSpec.extensions.has(ext)) {
    return true;
  }
  if (!ext) {
    return false;
  }

  const mime = guessMimeType(name);
  if (!mime) {
    return false;
  }
  if (filterSpec.exactMimeTypes.has(mime)) {
    return true;
  }

  const group = mime.substring(0, mime.indexOf('/'));
  return group.length > 0 && filterSpec.wildcardMimeGroups.has(group);
}

function listDirectoryEntries(
  currentDir: string,
  mode: LocalChooserMode,
  filterSpec: LocalFilterSpec
): LocalDirectoryListingResult {
  const names: string[] = fileIo.listFileSync(currentDir) as string[];
  const directories: LocalFileChooserEntry[] = [];
  const files: LocalFileChooserEntry[] = [];
  let hasFilteredOutFiles: boolean = false;

  names.forEach((name: string) => {
    const fullPath = `${trimSlashes(currentDir)}/${name}`;
    const isDirectory = canListDirectory(fullPath);
    if (isDirectory) {
      directories.push({
        path: fullPath,
        name,
        isDirectory: true,
        badge: '',
        subtitle: '',
      });
      return;
    }

    if (mode !== 'file') {
      return;
    }

    const extIndex = name.lastIndexOf('.');
    const ext = extIndex >= 0 ? name.substring(extIndex + 1).toLowerCase() : '';
    if (!matchesFileFilter(name, filterSpec)) {
      hasFilteredOutFiles = true;
      return;
    }

    let subtitle = '';
    try {
      const stat = fileIo.statSync(fullPath);
      const sizeText = formatFileSize(typeof stat.size === 'number' ? stat.size : 0);
      const timeText = relativeTimeLabel(typeof stat.mtime === 'number' ? stat.mtime : 0);
      subtitle = timeText ? `${sizeText} · ${timeText}` : sizeText;
    } catch (_) {
      subtitle = '';
    }

    files.push({
      path: fullPath,
      name,
      isDirectory: false,
      badge: ext ? ext.toUpperCase() : 'FILE',
      subtitle,
    });
  });

  directories.sort((a, b) => a.name.localeCompare(b.name));
  files.sort((a, b) => a.name.localeCompare(b.name));
  const result: LocalDirectoryListingResult = new LocalDirectoryListingResult();
  result.entries = directories.concat(files);
  result.hasFilteredOutFiles = hasFilteredOutFiles;
  return result;
}

@Component
export struct LocalFileChooserOverlay {
  @State private visible: boolean = LocalFileChooserOverlayManager.isShowing();
  @State private overlayState: LocalFileChooserOverlayState | null = LocalFileChooserOverlayManager.getState();
  @State private currentDir: string = '';
  @State private entries: LocalFileChooserEntry[] = [];
  @State private hasFilteredOutFiles: boolean = false;
  @State private revealedDeletePath: string = '';

  private listener = () => {
    this.visible = LocalFileChooserOverlayManager.isShowing();
    this.overlayState = LocalFileChooserOverlayManager.getState();
    if (this.visible && this.overlayState) {
      this.currentDir = this.overlayState.rootPath;
      this.reloadEntries();
    } else {
      this.currentDir = '';
      this.entries = [];
      this.hasFilteredOutFiles = false;
      this.revealedDeletePath = '';
    }
  }

  aboutToAppear() {
    LocalFileChooserOverlayManager.registerListener(this.listener);
    if (this.visible && this.overlayState) {
      this.currentDir = this.overlayState.rootPath;
      this.reloadEntries();
    }
  }

  aboutToDisappear() {
    LocalFileChooserOverlayManager.unregisterListener(this.listener);
  }

  private filterLabel(value: string): string {
    switch (value) {
      case 'image/*':
        return `${$r('app.string.lx_file_chooser_filter_images')}`;
      case 'video/*':
        return `${$r('app.string.lx_file_chooser_filter_videos')}`;
      case 'audio/*':
        return `${$r('app.string.lx_file_chooser_filter_audio')}`;
      default:
        return value.includes('/')
          ? value.substring(value.lastIndexOf('/') + 1).toUpperCase()
          : value.replace(/^\./, '').toUpperCase();
    }
  }

  private getFilterSpec(): LocalFilterSpec {
    const extensions: Set<string> = new Set();
    const exactMimeTypes: Set<string> = new Set();
    const wildcardMimeGroups: Set<string> = new Set();
    const labels: string[] = [];

    (this.overlayState?.filters ?? []).forEach((rawValue: string) => {
      const value = String(rawValue ?? '').trim().toLowerCase();
      if (!value) {
        return;
      }
      if (value.includes('/')) {
        if (value.endsWith('/*')) {
          wildcardMimeGroups.add(value.substring(0, value.indexOf('/')));
        } else {
          exactMimeTypes.add(value);
        }
      } else {
        extensions.add(value.replace(/^\./, ''));
      }
      labels.push(this.filterLabel(value));
    });

    return {
      extensions,
      exactMimeTypes,
      wildcardMimeGroups,
      labels,
    };
  }

  private reloadEntries(): void {
    if (!this.overlayState || !this.currentDir) {
      this.entries = [];
      this.hasFilteredOutFiles = false;
      return;
    }
    try {
      const result = listDirectoryEntries(this.currentDir, this.overlayState.mode, this.getFilterSpec());
      this.entries = result.entries;
      this.hasFilteredOutFiles = result.hasFilteredOutFiles;
    } catch (error) {
      hilog.error(DOMAIN, TAG, `reloadEntries failed: ${error}`);
      this.entries = [];
      this.hasFilteredOutFiles = false;
    }
  }

  private handleBack(): void {
    if (this.revealedDeletePath.length > 0) {
      this.revealedDeletePath = '';
      return;
    }
    if (!this.overlayState) {
      LocalFileChooserOverlayManager.cancel();
      return;
    }
    if (this.currentDir && this.currentDir !== this.overlayState.rootPath) {
      const parent = parentDirectory(this.currentDir);
      if (parent && isWithinRoot(parent, this.overlayState.rootPath)) {
        this.currentDir = parent;
        this.reloadEntries();
        return;
      }
    }
    LocalFileChooserOverlayManager.cancel();
  }

  private handleEntry(entry: LocalFileChooserEntry): void {
    if (!this.overlayState) {
      return;
    }
    if (this.revealedDeletePath.length > 0) {
      return;
    }
    if (entry.isDirectory) {
      this.currentDir = entry.path;
      this.reloadEntries();
      return;
    }
    if (this.overlayState.mode === 'file') {
      LocalFileChooserOverlayManager.complete([entry.path]);
    }
  }

  private requestDelete(entry: LocalFileChooserEntry): void {
    this.revealedDeletePath = entry.path;
  }

  private deletePathRecursive(path: string): boolean {
    try {
      if (canListDirectory(path)) {
        const names: string[] = fileIo.listFileSync(path) as string[];
        names.forEach((name: string) => {
          const child = `${trimSlashes(path)}/${name}`;
          this.deletePathRecursive(child);
        });
        fileIo.rmdirSync(path);
        return true;
      }
      fileIo.unlinkSync(path);
      return true;
    } catch (error) {
      hilog.error(DOMAIN, TAG, `deletePathRecursive failed: ${error}`);
      return false;
    }
  }

  private deleteRevealedPath(): void {
    const entry = this.entries.find((item: LocalFileChooserEntry) => item.path === this.revealedDeletePath) || null;
    this.revealedDeletePath = '';
    if (!entry || !this.overlayState) {
      return;
    }
    if (!isWithinRoot(entry.path, this.overlayState.rootPath)) {
      return;
    }
    const ok = this.deletePathRecursive(entry.path);
    if (!ok) {
      hilog.error(DOMAIN, TAG, 'delete failed');
    }
    this.reloadEntries();
  }

  private filterSummary(): string {
    const filterSpec = this.getFilterSpec();
    const labels = Array.from(new Set(filterSpec.labels));
    if (labels.length === 0) {
      return '';
    }
    return labels.length <= 3
      ? labels.join(' · ')
      : `${labels.slice(0, 3).join(' · ')} +${labels.length - 3}`;
  }

  @Builder
  private buildHeader(): void {
    Column() {
      Row()
        .width('100%')
        .height(48)
        .backgroundColor('#F8F8F8')

      Row() {
        Image($rawfile('icons/icon_back.svg'))
          .width(20)
          .height(20)
          .objectFit(ImageFit.Contain)
          .margin({ left: 4 })
          .onClick(() => this.handleBack())

        Text(this.overlayState?.title || $r('app.string.lx_file_chooser_default_title'))
          .layoutWeight(1)
          .fontSize(13)
          .fontWeight(FontWeight.Medium)
          .fontColor('#333333')
          .maxLines(1)
          .textOverflow({ overflow: TextOverflow.Ellipsis })
          .textAlign(TextAlign.Center)
          .margin({ left: 6, right: 6 })

        if (this.overlayState?.mode === 'directory') {
          Text($r('app.string.lx_common_done'))
            .fontSize(14)
            .fontWeight(FontWeight.Medium)
            .fontColor('#2563EB')
            .onClick(() => LocalFileChooserOverlayManager.complete([this.currentDir]))
        } else {
          Blank()
            .width(28)
        }
      }
      .width('100%')
      .height(44)
      .backgroundColor('#F8F8F8')
      .padding({ left: 8, right: 8, top: 4, bottom: 4 })
      .shadow({
        radius: 2,
        color: 'rgba(0, 0, 0, 0.03)',
        offsetX: 0,
        offsetY: 1
      })

      if (this.filterSummary().length > 0) {
        Row() {
          Text(`${$r('app.string.lx_file_chooser_filter_label')} `.replace('%1$s', this.filterSummary()))
            .fontSize(12)
            .fontColor('#2563EB')
        }
        .padding({ left: 16, right: 16, top: 10, bottom: 12 })
      }
    }
    .backgroundColor('#F8F8F8')
  }

  @Builder
  private buildEmptyState(): void {
    Column() {
      Text(this.hasFilteredOutFiles
        ? $r('app.string.lx_file_chooser_filtered_empty_title')
        : $r('app.string.lx_file_chooser_empty_title'))
        .fontSize(20)
        .fontWeight(FontWeight.Medium)
        .fontColor('#111827')

      Text(this.hasFilteredOutFiles
        ? $r('app.string.lx_file_chooser_filtered_empty_subtitle')
        : $r('app.string.lx_file_chooser_empty_subtitle'))
        .fontSize(14)
        .fontColor('#6B7280')
        .textAlign(TextAlign.Center)
        .margin({ top: 8 })
    }
    .width('100%')
    .padding({ left: 24, right: 24 })
    .justifyContent(FlexAlign.Center)
    .alignItems(HorizontalAlign.Center)
    .layoutWeight(1)
  }

  @Builder
  private buildEntry(entry: LocalFileChooserEntry): void {
    Row() {
      if (entry.isDirectory || entry.badge.length > 0) {
        Text(entry.isDirectory ? $r('app.string.lx_file_chooser_folder_subtitle') : entry.badge)
          .fontSize(11)
          .fontWeight(FontWeight.Medium)
          .fontColor(entry.isDirectory ? '#2563EB' : '#111827')
          .padding({ left: 10, right: 10, top: 6, bottom: 6 })
          .backgroundColor(entry.isDirectory ? '#E8F1FF' : '#EEF2F7')
          .borderRadius(12)
      }

      Column() {
        Text(entry.name)
          .fontSize(16)
          .fontWeight(FontWeight.Medium)
          .fontColor('#111827')
          .maxLines(1)
          .textOverflow({ overflow: TextOverflow.Ellipsis })

        if (!entry.isDirectory && entry.subtitle.length > 0) {
          Text(entry.subtitle)
            .fontSize(12)
            .fontColor('#6B7280')
            .margin({ top: 4 })
            .maxLines(1)
            .textOverflow({ overflow: TextOverflow.Ellipsis })
        }
      }
      .layoutWeight(1)
      .margin({ left: 12 })
      .alignItems(HorizontalAlign.Start)

      if (this.revealedDeletePath === entry.path) {
        Text($r('app.string.lx_common_delete'))
          .fontSize(14)
          .fontWeight(FontWeight.Medium)
          .fontColor('#DC2626')
          .onClick(() => this.deleteRevealedPath())
      } else if (entry.isDirectory) {
        Image($rawfile('icons/icon_forward.svg'))
          .width(18)
          .height(18)
          .objectFit(ImageFit.Contain)
      }
    }
    .width('100%')
    .padding({ left: 16, right: 16, top: 14, bottom: 14 })
    .backgroundColor('#FFFFFF')
    .border({
      width: { bottom: 1 },
      color: '#EEF2F7',
    })
    .onClick(() => this.handleEntry(entry))
    .gesture(
      LongPressGesture({ repeat: false, duration: 500 })
        .onAction(() => this.requestDelete(entry))
    )
  }

  build() {
    if (this.visible && this.overlayState) {
      Stack() {
        Column() {
          this.buildHeader()

          if (this.entries.length === 0) {
            this.buildEmptyState()
          } else {
            List() {
              ForEach(this.entries, (entry: LocalFileChooserEntry) => {
                ListItem() {
                  this.buildEntry(entry)
                }
              }, (entry: LocalFileChooserEntry) => entry.path)
            }
            .layoutWeight(1)
            .width('100%')
            .divider({ strokeWidth: 0, color: '#FFFFFF' })
            .backgroundColor('#FFFFFF')
          }
        }
        .width('100%')
        .height('100%')
      }
      .width('100%')
      .height('100%')
      .backgroundColor('#F8FAFC')
      .expandSafeArea([SafeAreaType.SYSTEM], [SafeAreaEdge.TOP, SafeAreaEdge.BOTTOM])
      .zIndex(10008)
    }
  }
}

export function resolveLocalFileChooserRoot(defaultPath: string): string | null {
  return resolveChooserRoot(defaultPath);
}

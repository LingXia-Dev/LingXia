/**
 * Media APIs
 * Corresponds to: lingxia-logic/src/media/
 */

export interface GetImageInfoOptions {
  path: string;
}

export interface ImageInfo {
  width: number;
  height: number;
  type: string;
  path: string;
}

export interface CompressImageOptions {
  path: string;
  quality?: number;
  compressedWidth?: number;
  compressedHeight?: number;
}

export interface CompressImageResult {
  tempFilePath: string;
}

export interface ChooseMediaOptions {
  count?: number;
  mediaType?: ('image' | 'video')[];
  sourceType?: ('album' | 'camera')[];
  camera?: 'back' | 'front';
  maxDuration?: number;
}

export interface ChosenMediaEntry {
  tempFilePath: string;
  fileType: 'image' | 'video';
  isOriginal: boolean;
}

export interface PreviewMediaItem {
  path: string;
  type?: 'image' | 'video';
  coverPath?: string;
}

export interface PreviewMediaOptions {
  sources: PreviewMediaItem[];
}

export interface SaveMediaOptions {
  filePath: string;
}

export interface ScanCodeOptions {
  onlyFromCamera?: boolean;
  scanType?: ('barCode' | 'qrCode' | 'datamatrix' | 'pdf417')[];
}

export interface ScanCodeResult {
  scanResult: string;
  scanType: string;
}

export interface StreamSourceOptions {
  provider: string;
  isLive: boolean;
  duration?: number;
  params?: Record<string, unknown>;
}

export interface VideoContext {
  play(): void;
  pause(): void;
  stop(): void;
  seek(position: number): void;
  requestFullScreen(): void;
  exitFullScreen(): void;
  setStreamSource(options: StreamSourceOptions): void;
}

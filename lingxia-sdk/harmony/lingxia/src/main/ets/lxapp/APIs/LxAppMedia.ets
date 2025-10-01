import { hilog } from '@kit.PerformanceAnalysisKit';
import { MediaPreviewOverlayManager, MediaPreviewItem, MediaPreviewPageParams } from '../MediaPreviewOverlay';

const DOMAIN = 0x0000;
const TAG = 'LingXia.Media';

interface PreviewMediaPayload {
  path: string;
  media_type: number;
  cover_url?: string;
}

export class LxAppMedia {
  public static previewMedia(payload: string): boolean {
    try {
      const decoded: string = typeof payload === 'string' ? payload.replace(/%7C/g, '|') : '';
      const parsed: PreviewMediaPayload[] = JSON.parse(decoded ?? '[]');
      if (!Array.isArray(parsed) || parsed.length === 0) {
        hilog.error(DOMAIN, TAG, 'previewMedia called with empty payload');
        return false;
      }

      const items: MediaPreviewItem[] = parsed
        .map((entry, index) => LxAppMedia.normalizeEntry(entry, index))
        .filter((item) => item !== null) as MediaPreviewItem[];

      if (items.length === 0) {
        hilog.error(DOMAIN, TAG, 'previewMedia normalized items are empty');
        return false;
      }

      hilog.info(DOMAIN, TAG, `previewMedia: items count=${items.length}`);
      items.forEach((item, idx) => {
        hilog.info(DOMAIN, TAG, `  Item ${idx}: id=${item.id}, uri=${item.uri}`);
      });

      const success = LxAppMedia.navigateToMediaPreview(items);
      if (!success) {
        hilog.error(DOMAIN, TAG, `previewMedia failed to navigate to media preview`);
      } else {
        hilog.info(DOMAIN, TAG, `previewMedia success`);
      }
      return success;
    } catch (error) {
      hilog.error(DOMAIN, TAG, `previewMedia error: ${error}`);
      return false;
    }
  }

  private static normalizeEntry(entry: PreviewMediaPayload, index: number): MediaPreviewItem | null {
    if (!entry || typeof entry.path !== 'string' || entry.path.length === 0) {
      return null;
    }

    const rawPath = entry.path.trim();
    const uri = LxAppMedia.normalizeUri(rawPath);
    if (!uri) {
      return null;
    }

    // Phase 1: Support image preview only (media_type: 0 = image, 1 = video)
    const mediaType = typeof entry.media_type === 'number' ? entry.media_type : Number(entry.media_type ?? 0);
    if (mediaType !== 0) {
      hilog.warn(DOMAIN, TAG, `previewMedia phase 1 supports images only, skipping media type: ${mediaType}`);
      return null;
    }

    return {
      id: `${Date.now()}_${index}`,
      uri: uri
    };
  }

  private static navigateToMediaPreview(items: MediaPreviewItem[]): boolean {
    try {
      const params: MediaPreviewPageParams = {
        items: items,
        initialIndex: 0
      };
      
      // Use overlay system
      MediaPreviewOverlayManager.show(params);
      hilog.info(DOMAIN, TAG, `Media preview overlay shown with ${items.length} items`);
      return true;
    } catch (error) {
      hilog.error(DOMAIN, TAG, `Error showing media preview overlay: ${error}`);
      return false;
    }
  }

  private static normalizeUri(path: string): string {
    if (!path) {
      return '';
    }
    if (path.startsWith('file://') || path.startsWith('content://') || path.startsWith('data:') || path.startsWith('http://') || path.startsWith('https://')) {
      return path;
    }
    return `file://${path}`;
  }
}

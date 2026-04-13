import { hilog } from '@kit.PerformanceAnalysisKit';
import { Want } from '@kit.AbilityKit';
import wantConstant from '@ohos.ability.wantConstant';
import common from '@ohos.app.ability.common';
import picker from '@ohos.file.picker';
import { LxApp } from '../LxApp';
import { PdfSingleViewerOverlayManager } from './document/PdfSinglePageViewerOverlay';
import { LocalFileChooserOverlayManager, resolveLocalFileChooserRoot } from './file/LocalFileChooserOverlay';
import { MediaPreviewOverlayManager, MediaPreviewPageParams, MediaPreviewItem } from './media/MediaPreviewOverlay';
import fileUri from '@ohos.file.fileuri';
import { systemShare } from '@kit.ShareKit';
import { uniformTypeDescriptor } from '@kit.ArkData';
import { BusinessError } from '@kit.BasicServicesKit';
import { onCallback } from 'liblingxia.so';

const DOMAIN = 0x0000;
const TAG = 'LingXia.File';
const ACTION_VIEW = 'ohos.want.action.viewData';

interface UriCreator {
  createUriFromPath?(path: string): string | undefined;
}

interface ValidatedRequest {
  context: common.UIAbilityContext;
  normalizedMime: string | undefined;
}

export class LxAppFile {
  private static validate(caller: string, filePath: string, mimeType: string): ValidatedRequest | undefined {
    if (!filePath || filePath.length === 0) {
      hilog.warn(DOMAIN, TAG, `${caller}: empty file path`);
      return undefined;
    }
    const context = LxApp.context;
    if (!context) {
      hilog.warn(DOMAIN, TAG, `${caller}: UIAbility context unavailable`);
      return undefined;
    }
    const normalizedMime = mimeType && mimeType.length > 0 ? mimeType : undefined;
    return { context, normalizedMime };
  }

  public static openDocument(filePath: string, mimeType: string = '', showMenu: boolean = true): boolean {
    return LxAppFile.reviewDocument(filePath, mimeType, showMenu) ||
      LxAppFile.openDocumentExternal(filePath, mimeType, showMenu);
  }

  public static reviewDocument(filePath: string, mimeType: string = '', showMenu: boolean = true): boolean {
    const req = LxAppFile.validate('reviewDocument', filePath, mimeType);
    if (!req) {
      return false;
    }

    const isPdf = (req.normalizedMime && req.normalizedMime.toLowerCase() === 'application/pdf') || filePath.toLowerCase().endsWith('.pdf');
    if (isPdf) {
      const title = getDisplayName(filePath);
      const shareHandler = () => shareDocument(req.context, filePath, req.normalizedMime);
      PdfSingleViewerOverlayManager.show(filePath, { title, showMenu, onShare: shareHandler });
      return true;
    }

    const isImage = (req.normalizedMime && req.normalizedMime.toLowerCase().startsWith('image/')) ||
      /\.(jpg|jpeg|png|webp|gif|bmp|heic|heif)$/i.test(filePath);
    if (isImage) {
      const item: MediaPreviewItem = {
        id: filePath,
        uri: filePath.startsWith('file://') ? filePath : `file://${filePath}`,
        mediaType: 0,
      };
      const params: MediaPreviewPageParams = {
        items: [item],
        initialIndex: 0,
        advance: 'manual',
        showIndexIndicator: false,
      };
      MediaPreviewOverlayManager.show(params);
      return true;
    }

    hilog.info(DOMAIN, TAG, `reviewDocument: no native review handler for ${filePath}`);
    return false;
  }

  public static openDocumentExternal(filePath: string, mimeType: string = '', showMenu: boolean = true): boolean {
    const req = LxAppFile.validate('openDocumentExternal', filePath, mimeType);
    if (!req) {
      return false;
    }

    launchSystemViewer(req.context, filePath, req.normalizedMime, showMenu);
    return true;
  }

  public static chooseFile(
    callbackId: string,
    multiple: string,
    title: string,
    defaultPath: string,
    filtersJson: string
  ): boolean {
    const context = LxApp.context;
    const id = String(callbackId ?? '').trim();
    if (!context || !id) {
      if (id) {
        onCallback(id, false, '1000');
      }
      return false;
    }

    const localRoot = String(multiple ?? '') !== '1' ? resolveLocalFileChooserRoot(defaultPath) : null;
    if (localRoot) {
      return LocalFileChooserOverlayManager.show({
        title: title && title.length > 0 ? title : '',
        rootPath: localRoot,
        mode: 'file',
        callbackId: id,
        filters: parseLocalChooserFilters(filtersJson),
      });
    }

    const options = new picker.DocumentSelectOptions();
    options.maxSelectNumber = String(multiple ?? '') === '1' ? 500 : 1;
    const filters = parseDocumentFilters(filtersJson);
    if (filters.length > 0) {
      options.fileSuffixFilters = filters;
    }

    const viewPicker = new picker.DocumentViewPicker(context);
    viewPicker.select(options)
      .then((paths: Array<string>) => {
        onCallback(id, true, JSON.stringify({
          canceled: !paths || paths.length === 0,
          paths: paths ?? []
        }));
      })
      .catch((_error: Object) => {
        onCallback(id, true, JSON.stringify({ canceled: true, paths: [] }));
      });
    return true;
  }

  public static chooseDirectory(
    callbackId: string,
    title: string,
    defaultPath: string
  ): boolean {
    const context = LxApp.context;
    const id = String(callbackId ?? '').trim();
    if (!context || !id) {
      if (id) {
        onCallback(id, false, '1000');
      }
      return false;
    }

    const localRoot = resolveLocalFileChooserRoot(defaultPath);
    if (localRoot) {
      return LocalFileChooserOverlayManager.show({
        title: title && title.length > 0 ? title : '',
        rootPath: localRoot,
        mode: 'directory',
        callbackId: id,
        filters: [],
      });
    }

    const options = new picker.DocumentSelectOptions();
    options.selectMode = picker.DocumentSelectMode.FOLDER;
    options.maxSelectNumber = 1;

    const viewPicker = new picker.DocumentViewPicker(context);
    viewPicker.select(options)
      .then((paths: Array<string>) => {
        onCallback(id, true, JSON.stringify({
          canceled: !paths || paths.length === 0,
          paths: paths ?? []
        }));
      })
      .catch((_error: Object) => {
        onCallback(id, true, JSON.stringify({ canceled: true, paths: [] }));
      });
    return true;
  }
}

function parseDocumentFilters(filtersJson: string): Array<string> {
  if (!filtersJson || filtersJson.trim().length === 0) {
    return [];
  }
  try {
    const parsed = JSON.parse(filtersJson) as Array<string>;
    return (parsed ?? [])
      .map((raw: string) => String(raw ?? '').trim().replace(/^\./, ''))
      .filter((raw: string) => raw.length > 0 && raw.indexOf('/') < 0);
  } catch (_error) {
    return [];
  }
}

function parseLocalChooserFilters(filtersJson: string): Array<string> {
  if (!filtersJson || filtersJson.trim().length === 0) {
    return [];
  }
  try {
    const parsed = JSON.parse(filtersJson) as Array<string>;
    return (parsed ?? [])
      .map((raw: string) => String(raw ?? '').trim())
      .filter((raw: string) => raw.length > 0);
  } catch (_error) {
    return [];
  }
}

function launchSystemViewer(context: common.UIAbilityContext, filePath: string, mimeType?: string, showMenu: boolean = true): void {
  const uri = resolveUri(context, filePath);
  // Build Want to open directly with WPS Office
  const want: Want = {
    action: ACTION_VIEW,
    uri: uri,
    flags: wantConstant.Flags.FLAG_AUTH_READ_URI_PERMISSION,
    bundleName: 'cn.wps.moffice_eng',
    abilityName: 'cn.wps.moffice.documenteditor.DocumentEditorActivity'
  };
  if (mimeType && mimeType.length > 0) {
    want.type = mimeType;
  }
  hilog.info(DOMAIN, TAG, `Opening with WPS: uri=${uri}, bundleName=${want.bundleName}`);
  context.startAbility(want).catch((err: Error) => {
    hilog.error(DOMAIN, TAG, `Failed to open with WPS: ${err.message}, trying system viewer`);
    // Fallback to system viewer if WPS is not available
    const fallbackWant: Want = {
      action: ACTION_VIEW,
      uri: uri,
      flags: wantConstant.Flags.FLAG_AUTH_READ_URI_PERMISSION
    };
    if (mimeType && mimeType.length > 0) {
      fallbackWant.type = mimeType;
    }
    context.startAbility(fallbackWant).catch((err2: Error) => {
      hilog.error(DOMAIN, TAG, `openDocument startAbility error: ${err2.message}`);
    });
  });
}

function shareDocument(context: common.UIAbilityContext, filePath: string, mimeType?: string): void {
  const fileName = getDisplayName(filePath);

  hilog.info(DOMAIN, TAG, `shareDocument: filePath=${filePath}, fileName=${fileName}, type=${mimeType}`);

  // Get URI from file path
  const uri = fileUri.getUriFromPath(filePath);
  hilog.info(DOMAIN, TAG, `shareDocument: converted URI=${uri}`);

  // Get UTD type based on file extension - use COMPOSITE_OBJECT for PDF
  let utdTypeId = uniformTypeDescriptor.getUniformDataTypeByFilenameExtension('.pdf', uniformTypeDescriptor.UniformDataType.COMPOSITE_OBJECT);
  hilog.info(DOMAIN, TAG, `shareDocument: UTD type=${utdTypeId}`);

  // Use ShareKit systemShare API - following official sample code pattern
  const shareData = new systemShare.SharedData({
    utd: utdTypeId,
    uri: uri,
    title: fileName,
    description: 'PDF Document'
  });

  const controller = new systemShare.ShareController(shareData);

  controller.on('dismiss', () => {
    hilog.info(DOMAIN, TAG, 'Share panel dismissed');
  });

  controller.show(context, {
    previewMode: systemShare.SharePreviewMode.DETAIL,
    selectionMode: systemShare.SelectionMode.SINGLE
  }).then(() => {
    hilog.info(DOMAIN, TAG, 'Share panel shown successfully');
  }).catch((err: Error) => {
    hilog.error(DOMAIN, TAG, `shareDocument error: ${err.message}, code: ${(err as BusinessError).code}`);
  });
}

function getDisplayName(filePath: string): string {
  try {
    const decoded = decodeURIComponent(filePath);
    const segments = decoded.split('/');
    return segments.length > 0 ? segments[segments.length - 1] : decoded;
  } catch (_) {
    const segments = filePath.split('/');
    return segments.length > 0 ? segments[segments.length - 1] : filePath;
  }
}

function resolveUri(context: common.UIAbilityContext, filePath: string): string {
  const creator = (context as UriCreator).createUriFromPath;
  if (creator && typeof creator === 'function') {
    try {
      const created = creator(filePath);
      if (created && created.length > 0) {
        return created;
      }
    } catch (error) {
      hilog.warn(DOMAIN, TAG, `createUriFromPath failed: ${error}`);
    }
  }
  // Try fileuri.getUriFromPath for datashare/file scheme
  try {
    const uri = fileUri.getUriFromPath(filePath);
    if (uri && uri.length > 0) { return uri; }
  } catch (error) {
    hilog.warn(DOMAIN, TAG, `fileuri.getUriFromPath failed: ${error}`);
  }
  // Fallback: ensure a scheme is present
  if (filePath.startsWith('file://') || filePath.startsWith('content://') || filePath.startsWith('datashare://')) {
    return filePath;
  }
  return `file://${filePath}`;
}

import { hilog } from '@kit.PerformanceAnalysisKit';
import { Want } from '@kit.AbilityKit';
import wantConstant from '@ohos.ability.wantConstant';
import common from '@ohos.app.ability.common';
import { LxApp } from '../LxApp';
import { PdfSingleViewerOverlayManager } from './document/PdfSinglePageViewerOverlay';
import fileUri from '@ohos.file.fileuri';
import { systemShare } from '@kit.ShareKit';
import { uniformTypeDescriptor } from '@kit.ArkData';
import { BusinessError } from '@kit.BasicServicesKit';

const DOMAIN = 0x0000;
const TAG = 'LingXia.Document';
const ACTION_VIEW = 'ohos.want.action.viewData';

interface UriCreator {
  createUriFromPath?(path: string): string | undefined;
}

export class LxAppDocument {
  public static openDocument(filePath: string, mimeType: string = '', showMenu: boolean = true): boolean {
    if (!filePath || filePath.length === 0) {
      hilog.warn(DOMAIN, TAG, 'openDocument: empty file path');
      return false;
    }

    const context = LxApp.context;
    if (!context) {
      hilog.warn(DOMAIN, TAG, 'openDocument: UIAbility context unavailable');
      return false;
    }

    const normalizedMime = mimeType && mimeType.length > 0 ? mimeType : undefined;

    // Closed SDK path: use built-in single-page viewer for PDFs
    const isPdf = (normalizedMime && normalizedMime.toLowerCase() === 'application/pdf') || filePath.toLowerCase().endsWith('.pdf');
    if (isPdf) {
      const title = getDisplayName(filePath);
      const shareHandler = () => shareDocument(context, filePath, normalizedMime);
      PdfSingleViewerOverlayManager.show(filePath, { title, showMenu, onShare: shareHandler });
      return true;
    }

    // Non-PDF: use system viewer with fileshare
    launchSystemViewer(context, filePath, normalizedMime);
    return true;
  }
}

function launchSystemViewer(context: common.UIAbilityContext, filePath: string, mimeType?: string): void {
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

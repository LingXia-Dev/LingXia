/**
 * Share APIs.
 */

export type ShareQuery = Record<string, string | number | boolean>;

export type SharePage =
  /**
   * Share the current page.
   */
  | true
  /**
   * Share the current page with query.
   */
  | {
      /**
       * Query appended to the current page. Query belongs to the page target and is
       * encoded into the AppLink URL.
       */
      query?: ShareQuery;
    };

interface ShareTitleOptions {
  /**
   * Share title.
   */
  title?: string;
}

interface ShareTextBaseOptions extends ShareTitleOptions {
  /**
   * Share text body.
   */
  text?: string;
}

/**
 * Share title/text only. Receiver support is platform/app dependent; some
 * share extensions may reject text-only shares.
 */
export interface ShareTextOptions extends ShareTextBaseOptions {
  page?: never;
  files?: never;
}

/**
 * Share the current page as an AppLink.
 */
export interface SharePageOptions extends ShareTextBaseOptions {
  /**
   * Share the current page. The runtime uses the current appId and page path
   * implicitly and shares it through the host AppLink configuration.
   *
   * Rejects when the host app has no `appLinks.hosts` configuration because
   * receivers would not be able to open the shared page.
   *
   * `title` and `text` are presentation hints. Platforms and receivers may
   * ignore them; on iOS the URL is shared by itself so receivers can render it
   * as a webpage card when they support that.
   *
   * `page` and `files` are mutually exclusive.
   */
  page: SharePage;
  files?: never;
}

/**
 * Share images, PDFs, or other files.
 */
export interface ShareFilesOptions extends ShareTitleOptions {
  /**
   * File paths returned by LingXia APIs to share. Images, PDFs, and other
   * documents are all represented as file paths.
   *
   * Use `lx.chooseFile` for system files and `lx.chooseMedia` for picked media;
   * pass the returned path here without parsing it.
   * Some platforms or receivers may limit multi-file shares. Share files one
   * at a time when targeting those receivers.
   *
   * `files` and `page` are mutually exclusive.
   * `text` is intentionally not supported for file shares because system
   * receivers handle text+attachment inconsistently.
   */
  files: string[];
  page?: never;
  text?: never;
}

export type ShareOptions = ShareTextOptions | SharePageOptions | ShareFilesOptions;

export interface ShareResult {
  /**
   * Best-effort completion flag. Some platforms can only confirm that the
   * system share UI was opened or closed.
   */
  completed?: boolean;
}

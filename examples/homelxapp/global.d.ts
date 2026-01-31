/**
 * Global type declarations for homelxapp
 */

/// <reference types="lingxia-types" />

// Import Rong types and make them globally available
import type * as RongFS from '@lingxia/rong/fs';
import type * as RongPath from '@lingxia/rong/path';
import type * as RongProcess from '@lingxia/rong/process';

declare global {
  namespace Rong {
    // File System APIs
    const writeTextFile: typeof RongFS.writeTextFile;
    const readTextFile: typeof RongFS.readTextFile;
    const mkdir: typeof RongFS.mkdir;
    const readDir: typeof RongFS.readDir;
    const remove: typeof RongFS.remove;
    const exists: typeof RongFS.exists;
    const stat: typeof RongFS.stat;
    const rename: typeof RongFS.rename;
    const copyFile: typeof RongFS.copyFile;

    // Path APIs
    const join: typeof RongPath.join;
    const dirname: typeof RongPath.dirname;
    const basename: typeof RongPath.basename;
    const extname: typeof RongPath.extname;

    // Process APIs (if needed)
    const exit: typeof RongProcess.exit;
    const cwd: typeof RongProcess.cwd;
  }
}

export {};

import { LxApp, LxAppNavigation } from 'lingxia';
import { hilog } from '@kit.PerformanceAnalysisKit';
import { fileIo } from '@kit.CoreFileKit';

const DOMAIN = 0x0000;
const TAG = 'LingXia.TestInstall';

/**
 * Install test lxapp for openLxApp functionality testing
 * This copies the installed homelxapp from data directory to create a test lxapp
 */
function installTestLxApp(): void {
  hilog.info(DOMAIN, TAG, 'Installing test lxapp: 95dc2dcfcccc191');

  try {
    const context = getContext();
    const appContext = context.getApplicationContext();
    const dataDir = appContext.filesDir;

    // Construct paths for source and destination
    const lingxiaDir = `${dataDir}/lingxia`;
    const lxappsDir = `${lingxiaDir}/lxapps`;
    const versionsDir = `${lingxiaDir}/versions`;

    const sourceDir = `${lxappsDir}/homelxapp`;

    // Use the exact hash from the error message for the test lxapp
    const testAppId = 'testlxapp';
    const hashedDirName = '95dc2dcfcccc191'; // Direct hash from error message

    const destDir = `${lxappsDir}/${hashedDirName}`;
    const sourceVersionFile = `${versionsDir}/homelxapp.txt`;
    const destVersionFile = `${versionsDir}/${testAppId}.txt`; // Version file uses original appId

    hilog.info(DOMAIN, TAG, `Using hash directory: ${hashedDirName} for appId: ${testAppId}`);

    // Check if source homelxapp exists
    if (!fileIo.accessSync(sourceDir)) {
      hilog.error(DOMAIN, TAG, `Source homelxapp not found at: ${sourceDir}`);
      return;
    }

    // Remove destination if it already exists
    if (fileIo.accessSync(destDir)) {
      fileIo.rmdirSync(destDir);
      hilog.info(DOMAIN, TAG, `Removed existing test lxapp directory: ${destDir}`);
    }

    // Create destination directory
    fileIo.mkdirSync(destDir);

    // Copy the entire homelxapp directory to create test lxapp
    copyDirectory(sourceDir, destDir);
    hilog.info(DOMAIN, TAG, `Successfully copied homelxapp to test lxapp directory: ${destDir}`);

    // Copy version file if it exists
    if (fileIo.accessSync(sourceVersionFile)) {
      if (fileIo.accessSync(destVersionFile)) {
        fileIo.unlinkSync(destVersionFile);
      }
      fileIo.copyFileSync(sourceVersionFile, destVersionFile);
      hilog.info(DOMAIN, TAG, 'Successfully copied version file');
    }

    hilog.info(DOMAIN, TAG, `Test lxapp ${testAppId} installation completed successfully in directory: ${hashedDirName}`);

  } catch (error) {
    hilog.error(DOMAIN, TAG, `Failed to install test lxapp: ${JSON.stringify(error)}`);
  }
}

/**
 * Copy directory recursively
 * @param src - Source directory path
 * @param dest - Destination directory path
 */
function copyDirectory(src: string, dest: string): void {
  try {
    const entries = fileIo.listFileSync(src);

    for (const entry of entries) {
      const srcPath = `${src}/${entry}`;
      const destPath = `${dest}/${entry}`;

      const stat = fileIo.statSync(srcPath);
      if (stat.isDirectory()) {
        fileIo.mkdirSync(destPath);
        copyDirectory(srcPath, destPath);
      } else {
        fileIo.copyFileSync(srcPath, destPath);
      }
    }
  } catch (error) {
    hilog.error(DOMAIN, TAG, `Failed to copy directory from ${src} to ${dest}: ${JSON.stringify(error)}`);
    return; // Don't throw, just return
  }
}

@Entry
@Component
struct Index {
  aboutToAppear() {
    // Install test lxapp for openLxApp testing
    //installTestLxApp();

    LxApp.openHomeLxApp();
  }

  build() {
    Column() {
      LxAppNavigation({ autoOpenHome: false })
    }
    .width('100%')
    .height('100%')
    .backgroundColor(Color.Transparent)
  }
}

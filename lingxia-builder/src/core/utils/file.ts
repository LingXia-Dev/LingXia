import fs from 'fs';
import path from 'path';

export class FileUtils {
  /**
   * Copy directory recursively
   */
  async copyDirectory(sourceDir: string, destDir: string): Promise<void> {
    if (!fs.existsSync(destDir)) {
      fs.mkdirSync(destDir, { recursive: true });
    }

    const entries = fs.readdirSync(sourceDir, { withFileTypes: true });

    for (const entry of entries) {
      const sourcePath = path.join(sourceDir, entry.name);
      const destPath = path.join(destDir, entry.name);

      if (entry.isDirectory()) {
        await this.copyDirectory(sourcePath, destPath);
      } else {
        fs.copyFileSync(sourcePath, destPath);
      }
    }
  }

  /**
   * Ensure directory exists
   */
  ensureDirectory(dirPath: string): void {
    if (!fs.existsSync(dirPath)) {
      fs.mkdirSync(dirPath, { recursive: true });
    }
  }

  /**
   * Clean directory (remove all contents)
   */
  cleanDirectory(dirPath: string): void {
    if (fs.existsSync(dirPath)) {
      fs.rmSync(dirPath, { recursive: true, force: true });
    }
    this.ensureDirectory(dirPath);
  }

  /**
   * Get file extension without dot
   */
  getExtension(filePath: string): string {
    return path.extname(filePath).slice(1);
  }

  /**
   * Get base name without extension
   */
  getBaseName(filePath: string): string {
    return path.basename(filePath, path.extname(filePath));
  }

  /**
   * Check if file exists and is readable
   */
  isReadableFile(filePath: string): boolean {
    try {
      fs.accessSync(filePath, fs.constants.R_OK);
      return fs.statSync(filePath).isFile();
    } catch {
      return false;
    }
  }

  /**
   * Read JSON file safely
   */
  readJsonFile<T = any>(filePath: string): T | null {
    try {
      const content = fs.readFileSync(filePath, 'utf-8');
      return JSON.parse(content) as T;
    } catch {
      return null;
    }
  }

  /**
   * Write JSON file with formatting
   */
  writeJsonFile(filePath: string, data: any): void {
    const content = JSON.stringify(data, null, 2);
    fs.writeFileSync(filePath, content, 'utf-8');
  }
}

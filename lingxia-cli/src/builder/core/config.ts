import * as fs from 'fs';
import * as path from 'path';

/**
 * Centralized configuration manager for LingXia projects
 * Handles lxapp.json and other configuration files
 */
export class ConfigManager {
  private projectPath: string;
  private lxappConfig: any | null = null;

  constructor(projectPath: string) {
    this.projectPath = projectPath;
  }

  /**
   * Read and cache lxapp.json configuration
   */
  getLxappConfig(): any {
    if (this.lxappConfig === null) {
      const lxappPath = path.join(this.projectPath, 'lxapp.json');
      if (!fs.existsSync(lxappPath)) {
        throw new Error('lxapp.json not found in project root');
      }
      this.lxappConfig = JSON.parse(fs.readFileSync(lxappPath, 'utf-8'));
    }
    return this.lxappConfig;
  }

  /**
   * Get pages configuration from lxapp.json
   */
  getPages(): string[] {
    const config = this.getLxappConfig();
    return config.pages || [];
  }


  /**
   * Check if project has package.json
   */
  hasPackageJson(): boolean {
    return fs.existsSync(path.join(this.projectPath, 'package.json'));
  }

  /**
   * Read package.json if exists
   */
  getPackageJson(): any | null {
    const packagePath = path.join(this.projectPath, 'package.json');
    if (fs.existsSync(packagePath)) {
      return JSON.parse(fs.readFileSync(packagePath, 'utf-8'));
    }
    return null;
  }

}

import * as fs from "fs";
import * as path from "path";
import type { LxPluginConfig } from "../types/index.js";

/**
 * Centralized configuration manager for LingXia projects
 * Handles lxapp.json, lxplugin.json, and other configuration files
 */
export class ConfigManager {
  private projectPath: string;
  private lxappConfig: any | null = null;
  private lxpluginConfig: LxPluginConfig | null | undefined = undefined;

  constructor(projectPath: string) {
    this.projectPath = projectPath;
  }

  /**
   * Read and cache lxapp.json configuration
   */
  getLxappConfig(): any {
    if (this.lxappConfig === null) {
      const lxappPath = path.join(this.projectPath, "lxapp.json");
      if (!fs.existsSync(lxappPath)) {
        throw new Error("lxapp.json not found in project root");
      }
      this.lxappConfig = JSON.parse(fs.readFileSync(lxappPath, "utf-8"));
    }
    return this.lxappConfig;
  }

  /**
   * Read and cache lxplugin.json configuration
   */
  getLxpluginConfig(): LxPluginConfig | null {
    if (this.lxpluginConfig === undefined) {
      const lxpluginPath = path.join(this.projectPath, "lxplugin.json");
      if (!fs.existsSync(lxpluginPath)) {
        this.lxpluginConfig = null;
      } else {
        this.lxpluginConfig = JSON.parse(
          fs.readFileSync(lxpluginPath, "utf-8"),
        ) as LxPluginConfig;
      }
    }
    return this.lxpluginConfig;
  }

  /**
   * Get pages configuration from lxapp.json (default) or lxplugin.json (plugin mode)
   */
  getPages(options: { plugin?: boolean } = {}): string[] {
    if (options.plugin) {
      const pluginConfig = this.getLxpluginConfig();
      if (!pluginConfig) {
        throw new Error("lxplugin.json not found in project root");
      }
      return Object.values(pluginConfig.pages);
    }

    const config = this.getLxappConfig();
    return Array.isArray(config.pages) ? config.pages : [];
  }

  /**
   * Check if project has package.json
   */
  hasPackageJson(): boolean {
    return fs.existsSync(path.join(this.projectPath, "package.json"));
  }

  /**
   * Read package.json if exists
   */
  getPackageJson(): any | null {
    const packagePath = path.join(this.projectPath, "package.json");
    if (fs.existsSync(packagePath)) {
      return JSON.parse(fs.readFileSync(packagePath, "utf-8"));
    }
    return null;
  }
}

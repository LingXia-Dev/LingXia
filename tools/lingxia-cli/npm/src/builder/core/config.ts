import * as fs from "fs";
import * as path from "path";
import type { LxPluginConfig } from "../types/index.js";

function isSafeLogicEntry(entry: string): boolean {
  if (!entry || entry.includes("\\")) {
    return false;
  }

  const normalized = path.posix.normalize(entry);
  if (normalized === "." || normalized.startsWith("../") || normalized.includes("/../")) {
    return false;
  }

  return !path.posix.isAbsolute(normalized);
}

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

  getLogicEntry(): string | null {
    const config = this.getLxappConfig();
    if (Object.prototype.hasOwnProperty.call(config, "appService")) {
      throw new Error('"appService" is no longer supported; use "logic" instead');
    }
    if (config.logic === false) {
      return null;
    }
    if (config.logic === true || config.logic === undefined || config.logic === null) {
      return "logic.js";
    }
    if (typeof config.logic === "string") {
      const logicEntry = config.logic.trim();
      if (!logicEntry) {
        throw new Error('"logic" entry must not be empty');
      }
      if (!isSafeLogicEntry(logicEntry)) {
        throw new Error(
          `"logic" entry must stay within the lxapp package: ${JSON.stringify(logicEntry)}`,
        );
      }
      return logicEntry;
    }
    throw new Error(
      `"logic" must be false, true, a string entry path, or omitted`,
    );
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

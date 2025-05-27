#!/usr/bin/env node

/**
 * LingXia MiniApp Builder CLI
 * Command line interface for building MiniApp packages
 */

import fs from "fs";
import path from "path";
import { LingXiaMiniAppBuilder } from "./vite-plugin.js";

/**
 * Build MiniApp package
 * @param {Object} options - Build options
 */
export async function buildMiniApp(options = {}) {
  const {
    configFile = "app.json",
    outputFile = "logic.js",
    buildDir = "dist",
    assetDirs = ["images"],
    minifyCode = true,
    removeComments = true,
    createPackage = true,
    packageName = null,
    appId = null,
    targetDir = null,
    copyToTarget = false,
  } = options;

  console.log("🚀 LingXia MiniApp Builder CLI");
  console.log("Building MiniApp package...\n");

  try {
    // Create a mock Vite plugin context
    const plugin = LingXiaMiniAppBuilder({
      configFile,
      outputFile,
      buildDir,
      assetDirs,
      minifyCode,
      removeComments,
      createPackage,
      packageName,
      appId,
      targetDir,
      copyToTarget,
    });

    // Simulate plugin execution
    plugin.configResolved({ root: process.cwd() });
    await plugin.buildStart();

    console.log("\n✅ Build completed successfully!");
  } catch (error) {
    console.error("\n❌ Build failed:", error.message);
    process.exit(1);
  }
}

// CLI entry point
if (import.meta.url === `file://${process.argv[1]}`) {
  const args = process.argv.slice(2);
  const options = {};

  // Parse command line arguments
  for (let i = 0; i < args.length; i += 2) {
    const key = args[i]?.replace(/^--/, "");
    const value = args[i + 1];

    if (key && value !== undefined) {
      // Convert string values to appropriate types
      if (value === "true") options[key] = true;
      else if (value === "false") options[key] = false;
      else if (!isNaN(value)) options[key] = Number(value);
      else options[key] = value;
    }
  }

  buildMiniApp(options);
}


#!/usr/bin/env node

/**
 * Generate update manifest (latest.json) for Tauri updater
 *
 * Usage:
 *   node scripts/generate-update-manifest.mjs <version> <notes>
 *
 * Example:
 *   node scripts/generate-update-manifest.mjs 0.1.0 "Initial release"
 */

import fs from "fs";
import path from "path";
import { fileURLToPath } from "url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const projectRoot = path.resolve(__dirname, "..");

const version = process.argv[2] || "0.1.0";
const notes = process.argv[3] || `Version ${version}`;
const pubDate = new Date().toISOString();

// Read signature files
const bundleDir = path.join(projectRoot, "src-tauri/target/release/bundle");

let windowsSig = "";
let linuxSig = "";

try {
  const windowsSigPath = path.join(
    bundleDir,
    `nsis/worktime-pomodoro_${version}_x64-setup.nsis.zip.sig`,
  );
  if (fs.existsSync(windowsSigPath)) {
    windowsSig = fs.readFileSync(windowsSigPath, "utf8").trim();
  }
} catch (err) {
  console.warn("Warning: Windows signature not found");
}

try {
  const linuxSigPath = path.join(
    bundleDir,
    `appimage/worktime-pomodoro_${version}_amd64.AppImage.tar.gz.sig`,
  );
  if (fs.existsSync(linuxSigPath)) {
    linuxSig = fs.readFileSync(linuxSigPath, "utf8").trim();
  }
} catch (err) {
  console.warn("Warning: Linux signature not found");
}

const manifest = {
  version,
  notes,
  pub_date: pubDate,
  platforms: {},
};

if (windowsSig) {
  manifest.platforms["windows-x86_64"] = {
    signature: windowsSig,
    url: `https://github.com/jeby-228/worktime-pomodoro-releases/releases/download/v${version}/worktime-pomodoro_${version}_x64-setup.nsis.zip`,
  };
}

if (linuxSig) {
  manifest.platforms["linux-x86_64"] = {
    signature: linuxSig,
    url: `https://github.com/jeby-228/worktime-pomodoro-releases/releases/download/v${version}/worktime-pomodoro_${version}_amd64.AppImage.tar.gz`,
  };
}

const outputPath = path.join(projectRoot, "latest.json");
fs.writeFileSync(outputPath, JSON.stringify(manifest, null, 2));

console.log(`✅ Generated ${outputPath}`);
console.log(JSON.stringify(manifest, null, 2));

if (!windowsSig && !linuxSig) {
  console.error("\n❌ No signatures found. Build the project first with signing enabled.");
  process.exit(1);
}

#!/usr/bin/env node
// Build a standalone binary using esbuild (bundle) + Node.js SEA (Single Executable Application)
// This approach works on macOS with code signing (unlike pkg)

import { execSync } from 'node:child_process';
import { writeFileSync, readFileSync, copyFileSync, mkdirSync, existsSync } from 'node:fs';

mkdirSync('release', { recursive: true });

// Step 1: Bundle with esbuild into single CJS file
// node-pty must be external — it's a native module loaded at runtime
console.log('Step 1: Bundling with esbuild...');
execSync(
  'npx esbuild src/cli.ts --bundle --platform=node --format=cjs ' +
  '--outfile=dist/rtb-bundle.cjs ' +
  '--external:node-pty ' +
  '--external:@larksuiteoapi/node-sdk ' +
  '--define:import.meta.url=__import_meta_url',
  { stdio: 'inherit' }
);

// Prepend banner: shims for SEA environment
// 1. import.meta.url shim for __dirname resolution
// 2. Override require() to use module.createRequire() so native modules load from disk
const banner = `
var __import_meta_url = typeof document === "undefined" ? require("url").pathToFileURL(__filename || process.execPath).href : "";
if (require("node:module").isBuiltin === void 0 || require("node:sea") !== void 0) {
  var _origRequire = require;
  require = require("node:module").createRequire(process.execPath);
  require.main = _origRequire.main;
}
`.trim();
const bundle = readFileSync('dist/rtb-bundle.cjs', 'utf-8');
writeFileSync('dist/rtb-bundle.cjs', banner + '\n' + bundle);

// Step 2: Create SEA config
console.log('Step 2: Creating SEA blob...');
const seaConfig = {
  main: 'dist/rtb-bundle.cjs',
  output: 'dist/sea-prep.blob',
  disableExperimentalSEAWarning: true,
  useSnapshot: false,
  useCodeCache: true,
  assets: {},
};

// Include web assets in SEA
const webFiles = ['web/index.html', 'web/commands.json', 'web/sw.js'];
for (const f of webFiles) {
  if (existsSync(f)) {
    seaConfig.assets[f] = f;
  }
}

writeFileSync('dist/sea-config.json', JSON.stringify(seaConfig, null, 2));
execSync('node --experimental-sea-config dist/sea-config.json', { stdio: 'inherit' });

// Step 3: Create the executable
console.log('Step 3: Creating executable...');
const nodeBin = process.execPath;
const outputBin = 'release/rtb';
copyFileSync(nodeBin, outputBin);

// Remove signature on macOS (required before injecting blob)
if (process.platform === 'darwin') {
  execSync(`codesign --remove-signature ${outputBin}`, { stdio: 'inherit' });
}

// Inject the SEA blob
execSync(
  `npx postject ${outputBin} NODE_SEA_BLOB dist/sea-prep.blob ` +
  '--sentinel-fuse NODE_SEA_FUSE_fce680ab2cc467b6e072b8b5df1996b2 ' +
  (process.platform === 'darwin' ? '--macho-segment-name NODE_SEA ' : ''),
  { stdio: 'inherit' }
);

// Re-sign on macOS
if (process.platform === 'darwin') {
  execSync(`codesign --sign - ${outputBin}`, { stdio: 'inherit' });
}

console.log('Done! Binary at release/rtb');

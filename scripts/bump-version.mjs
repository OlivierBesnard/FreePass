// Bump the version in the three files that must stay in sync, then print the
// git commands to cut a release. Usage: node scripts/bump-version.mjs 0.1.1
import { readFileSync, writeFileSync } from "node:fs";

const version = process.argv[2];
if (!/^\d+\.\d+\.\d+$/.test(version || "")) {
  console.error("usage: node scripts/bump-version.mjs X.Y.Z");
  process.exit(1);
}

function patch(path, re, replacement) {
  const before = readFileSync(path, "utf8");
  const after = before.replace(re, replacement);
  if (before === after) {
    console.error(`! version inchangée dans ${path} (motif introuvable)`);
    process.exit(1);
  }
  writeFileSync(path, after);
  console.log(`  ${path}`);
}

patch("package.json", /("version":\s*")[^"]+(")/, `$1${version}$2`);
patch("src-tauri/tauri.conf.json", /("version":\s*")[^"]+(")/, `$1${version}$2`);
patch("src-tauri/Cargo.toml", /^version = "[^"]+"/m, `version = "${version}"`);
// Keep the extension manifest in lockstep so users can tell which build they run.
patch("extension/manifest.json", /("version":\s*")[^"]+(")/, `$1${version}$2`);

console.log(`\nVersion → ${version}. Pour publier :`);
console.log(`  git commit -am "release: v${version}"`);
console.log(`  git tag v${version}`);
console.log(`  git push --follow-tags`);
console.log(`\nGitHub Actions construit, signe et publie. Les users sont notifiés dans l'app.`);

import { readFileSync, writeFileSync } from "node:fs";
import { spawnSync } from "node:child_process";

const version = process.argv[2];
if (!version || !/^\d+\.\d+\.\d+(-[\w.]+)?$/.test(version)) {
  console.error("Usage: npm run bump -- <semver>");
  console.error("Example: npm run bump -- 0.2.0");
  process.exit(1);
}

function bumpJson(path) {
  const raw = readFileSync(path, "utf8");
  const obj = JSON.parse(raw);
  obj.version = version;
  const trailing = raw.endsWith("\n") ? "\n" : "";
  writeFileSync(path, JSON.stringify(obj, null, 2) + trailing);
}

function bumpCargoToml(path) {
  const raw = readFileSync(path, "utf8");
  const updated = raw.replace(/^(version\s*=\s*)"[^"]*"/m, `$1"${version}"`);
  if (updated === raw) {
    throw new Error(`Could not find a version line in ${path}`);
  }
  writeFileSync(path, updated);
}

bumpJson("package.json");
bumpJson("src-tauri/tauri.conf.json");
bumpCargoToml("src-tauri/Cargo.toml");

console.log(`Bumped package.json, tauri.conf.json, Cargo.toml to ${version}.`);

// Refresh Cargo.lock so CI stays in sync. Soft-fail if cargo isn't on PATH.
const cargo = spawnSync(
  "cargo",
  ["update", "--manifest-path", "src-tauri/Cargo.toml", "-p", "games-launcher"],
  { stdio: "inherit", shell: true },
);
if (cargo.status !== 0) {
  console.warn(
    "\ncargo update failed or cargo not found — run this before tagging:",
  );
  console.warn(
    "  cargo update --manifest-path src-tauri/Cargo.toml -p games-launcher",
  );
}

console.log(`
Next:
  git add package.json src-tauri/tauri.conf.json src-tauri/Cargo.toml src-tauri/Cargo.lock
  git commit -m "Bump version to ${version}"
  git push
  git tag v${version}
  git push origin v${version}
`);

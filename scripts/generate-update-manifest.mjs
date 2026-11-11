// 릴리스 파일과 업데이트 목록(latest.json)을 dist/hangulbond-release/<version>/에 모은다.
// macOS 파일은 이 Mac의 빌드 결과에서, Windows 설치 파일은 --windows <폴더>에서 가져온다.
// Windows 폴더에는 Windows에서 빌드한 *_x64-setup.exe와, 이 Mac에서 updater 키로 만든 .sig가 있어야 한다.
import { cp, mkdir, readFile, readdir, rm, writeFile } from "node:fs/promises";
import { basename, join, resolve } from "node:path";
import { pathToFileURL } from "node:url";

const root = new URL("../", import.meta.url);
const config = JSON.parse(await readFile(new URL("src-tauri/tauri.conf.json", root), "utf8"));
const version = config.version;
const productName = "HangulBond";
const baseUrl = `https://github.com/da-itda/mac-nfd-nfc-hangulbond/releases/download/v${version}`;
const bundleRoot = new URL("src-tauri/target/release/bundle/", root);
const macosBundle = new URL("macos/", bundleRoot);
const dmgBundle = new URL("dmg/", bundleRoot);
const outRoot = new URL(`dist/hangulbond-release/${version}/`, root);
const latestJsonUrl = new URL("latest.json", outRoot);
const windowsArg = process.argv.indexOf("--windows");
const windowsSource =
  windowsArg >= 0 ? pathToFileURL(`${resolve(process.argv[windowsArg + 1])}/`) : null;

const tarball = await findFile(macosBundle, ".app.tar.gz");
const signaturePath = `${tarball}.sig`;
const signature = (await readFile(signaturePath, "utf8")).trim();
const dmg = await findFile(dmgBundle, "_aarch64.dmg");

const macDir = new URL("darwin-aarch64/", outRoot);
await rm(outRoot, { recursive: true, force: true });
await mkdir(macDir, { recursive: true });

const updateFileName = `${productName}_${version}_darwin_aarch64.app.tar.gz`;
const signatureFileName = `${updateFileName}.sig`;
const dmgFileName = `${productName}_${version}_aarch64.dmg`;

await cp(tarball, new URL(updateFileName, macDir));
await cp(signaturePath, new URL(signatureFileName, macDir));
await cp(dmg, new URL(dmgFileName, macDir));

let windowsPlatforms = {};
if (windowsSource) {
  const setup = await findFile(windowsSource, "_x64-setup.exe");
  const windowsSignature = (await readFile(`${setup}.sig`, "utf8")).trim();
  const windowsDir = new URL("windows-x86_64/", outRoot);
  const setupFileName = `${productName}_${version}_x64-setup.exe`;
  await mkdir(windowsDir, { recursive: true });
  await cp(setup, new URL(setupFileName, windowsDir));
  await cp(`${setup}.sig`, new URL(`${setupFileName}.sig`, windowsDir));
  const entry = { signature: windowsSignature, url: `${baseUrl}/${setupFileName}` };
  windowsPlatforms = { "windows-x86_64-nsis": entry, "windows-x86_64": entry };
}

const latest = {
  version,
  notes: `한글본드 ${version}`,
  pub_date: new Date().toISOString(),
  platforms: {
    "darwin-aarch64-app": {
      signature,
      url: `${baseUrl}/${updateFileName}`
    },
    "darwin-aarch64": {
      signature,
      url: `${baseUrl}/${updateFileName}`
    },
    ...windowsPlatforms
  }
};

await writeFile(latestJsonUrl, `${JSON.stringify(latest, null, 2)}\n`);

console.log(`Prepared release files in ${outRoot.pathname}`);
console.log(
  `Attach latest.json and the files in ${windowsSource ? "darwin-aarch64/ and windows-x86_64/" : "darwin-aarch64/"} to GitHub Release v${version}.`
);

async function findFile(directoryUrl, suffix) {
  const entries = await readdir(directoryUrl);
  const match = entries.find((entry) => entry.endsWith(suffix));
  if (!match) {
    throw new Error(`Could not find *${suffix} in ${directoryUrl.pathname}`);
  }

  return join(directoryUrl.pathname, basename(match));
}

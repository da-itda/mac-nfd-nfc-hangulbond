// 한글본드 브라우저 확장 E2E 테스트
// 실제 확장을 설치한 Chrome으로 업로드를 재현하고, 서버가 받은 파일명이 NFC인지 확인한다.
// 맥 Chromium 파일 선택창이 넘기는 형태와 같게 NFD 경로를 넘긴다.
// 다운로드는 서버가 NFD 이름을 보낼 때 디스크에 저장된 이름을 확인한다.
//
// 실행: npm run test:extension
// 설치된 Chrome을 임시 프로필로 띄우고, 공식 디버깅 명령(Extensions.loadUnpacked)으로 확장을 설치한다.
// 평소 쓰는 Chrome 프로필은 건드리지 않는다. 다른 Chrome을 쓰려면 CHROME_PATH를 지정한다.
// 웹 스토어에서 받은 패키지를 풀어 검사하려면 EXTENSION_DIR에 그 폴더를 지정한다.
// 권한 요청 창은 자동으로 누를 수 없어서, 다운로드 기능을 켠 상태는 downloads 권한을 미리 넣은 사본으로 검사한다.
import { spawn } from "node:child_process";
import { cpSync, mkdirSync, mkdtempSync, readdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { createServer } from "node:http";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";

const chromePath = process.env.CHROME_PATH || "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome";

const extensionDir = process.env.EXTENSION_DIR || fileURLToPath(new URL("../extension/", import.meta.url));
const workDir = mkdtempSync(join(tmpdir(), "hangulbond-ext-e2e-"));
const nfdName = "보고서_최종본.txt".normalize("NFD");
const nfcName = "회의록_정리.txt".normalize("NFC");
const nfdPath = join(workDir, nfdName);
const nfcPath = join(workDir, nfcName);
writeFileSync(nfdPath, "nfd");
writeFileSync(nfcPath, "nfc");
const folderPath = join(workDir, "업로드폴더".normalize("NFD"));
mkdirSync(folderPath);
writeFileSync(join(folderPath, nfdName), "nfd");

// 다운로드 기능을 켠 상태: optional_permissions의 downloads를 처음부터 허용한 사본
const grantedDir = join(workDir, "extension-downloads");
cpSync(extensionDir, grantedDir, { recursive: true });
const grantedManifest = JSON.parse(readFileSync(join(grantedDir, "manifest.json"), "utf8"));
grantedManifest.permissions = [...(grantedManifest.permissions || []), ...grantedManifest.optional_permissions];
delete grantedManifest.optional_permissions;
writeFileSync(join(grantedDir, "manifest.json"), JSON.stringify(grantedManifest));

// 서버가 다운로드 이름을 보내는 방식별 응답. 옛날 방식(filename="...")과 주소 경로는 Chrome이 NFD를 그대로 저장한다.
const latin1 = (name) => Buffer.from(name).toString("latin1");
const downloads = {
  legacy: { name: "나_옛날방식.txt".normalize("NFD"), header: (name) => `attachment; filename="${latin1(name)}"` },
  nfc: { name: "라_정상이름.txt".normalize("NFC"), header: (name) => `attachment; filename="${latin1(name)}"` }
};
const pathDownloadName = "마_주소경로.txt".normalize("NFD");

const PAGE = `<!doctype html><meta charset="utf-8"><title>e2e</title>
<style>#drop{position:fixed;left:0;top:0;width:400px;height:200px;border:1px dashed #999}
body{margin:0;padding-top:220px}</style>
<div id="drop">drop</div>
<input id="js" type="file" multiple>
<form id="form" method="post" action="/upload?case=form" enctype="multipart/form-data">
  <input id="formInput" type="file" name="file" multiple onchange="this.form.submit()">
</form>
<input id="folder" type="file" webkitdirectory multiple>
<shadow-upload></shadow-upload>
<script>
const post = (name, files, extra = {}) => {
  const body = new FormData();
  for (const file of files) body.append("file", file);
  for (const [key, value] of Object.entries(extra)) body.append(key, value);
  return fetch("/upload?case=" + name, { method: "POST", body });
};
document.getElementById("js").addEventListener("change", (e) => post("js", e.target.files));
document.getElementById("folder").addEventListener("change", (e) => post("folder", e.target.files));
const drop = document.getElementById("drop");
drop.addEventListener("dragover", (e) => e.preventDefault());
drop.addEventListener("drop", (e) => {
  e.preventDefault();
  post("drop", e.dataTransfer.files, {
    text: e.dataTransfer.getData("text/plain"),
    trusted: String(e.isTrusted)
  });
});
customElements.define("shadow-upload", class extends HTMLElement {
  connectedCallback() {
    const input = document.createElement("input");
    input.type = "file";
    input.addEventListener("change", () => post("shadow", input.files));
    this.attachShadow({ mode: "open" }).append(input);
  }
});
</script>`;

const results = new Map();
const server = createServer((req, res) => {
  if (process.env.E2E_DEBUG) console.error("요청:", req.method, req.url);
  const url = new URL(req.url, "http://127.0.0.1");
  const download = downloads[url.pathname.slice("/download/".length)];
  if (url.pathname.startsWith("/download/") && download) {
    res.writeHead(200, { "Content-Type": "application/octet-stream", "Content-Disposition": download.header(download.name) });
    res.end("download");
    return;
  }
  if (url.pathname.startsWith("/file/")) {
    res.writeHead(200, { "Content-Type": "application/octet-stream" });
    res.end("download");
    return;
  }
  if (req.method === "POST" && url.pathname === "/upload") {
    const chunks = [];
    req.on("data", (chunk) => chunks.push(chunk));
    req.on("end", () => {
      const body = Buffer.concat(chunks).toString("utf8");
      const names = [...body.matchAll(/filename="([^"]*)"/g)].map((m) => m[1]);
      const field = (key) => body.match(new RegExp(`name="${key}"\\r\\n\\r\\n([^\\r]*)`))?.[1];
      results.set(url.searchParams.get("case"), { names, text: field("text"), trusted: field("trusted") });
      res.end("ok");
    });
    return;
  }
  res.setHeader("Content-Type", "text/html; charset=utf-8");
  res.end(PAGE);
});
await new Promise((resolve) => server.listen(0, "127.0.0.1", resolve));
const pageUrl = `http://127.0.0.1:${server.address().port}/`;

const form = (name) => (name === name.normalize("NFC") ? "NFC" : "NFD");

async function launch(extension) {
  const profile = mkdtempSync(join(workDir, "profile-"));
  const downloadDir = join(profile, "downloads");
  mkdirSync(join(profile, "Default"), { recursive: true });
  mkdirSync(downloadDir);
  writeFileSync(
    join(profile, "Default", "Preferences"),
    JSON.stringify({ download: { default_directory: downloadDir, prompt_for_download: false, directory_upgrade: true } })
  );
  const args = [
    "--headless=new",
    "--no-first-run",
    "--no-default-browser-check",
    `--user-data-dir=${profile}`,
    "--remote-debugging-pipe",
    "--enable-unsafe-extension-debugging"
  ];
  // fd 3으로 명령을 보내고 fd 4로 응답을 받는다. 메시지는 NUL 문자로 구분한다.
  const chrome = spawn(chromePath, [...args, "about:blank"], { stdio: ["ignore", "ignore", "ignore", "pipe", "pipe"] });
  const [toChrome, fromChrome] = [chrome.stdio[3], chrome.stdio[4]];

  let id = 0;
  const pending = new Map();
  let buffer = "";
  fromChrome.on("data", (chunk) => {
    buffer += chunk.toString("utf8");
    let end;
    while ((end = buffer.indexOf("\0")) >= 0) {
      const data = JSON.parse(buffer.slice(0, end));
      buffer = buffer.slice(end + 1);
      pending.get(data.id)?.(data);
    }
  });
  const call = (method, params = {}, sessionId) =>
    new Promise((resolve, reject) => {
      const timer = setTimeout(() => reject(new Error(`${method}: 응답 없음`)), 10000);
      pending.set(++id, (data) => {
        clearTimeout(timer);
        data.error ? reject(new Error(`${method}: ${data.error.message}`)) : resolve(data.result);
      });
      toChrome.write(JSON.stringify({ id, method, params, sessionId }) + "\0");
    });

  if (extension) {
    const { id: extensionId } = await call("Extensions.loadUnpacked", { path: extension });
    // 설치 직후에는 서비스 워커(background.js)가 아직 뜨지 않았을 수 있다. 뜰 때까지 기다린다.
    for (let i = 0; i < 50; i += 1) {
      const { targetInfos } = await call("Target.getTargets");
      if (targetInfos.some((t) => t.type === "service_worker" && t.url.includes(extensionId))) break;
      await sleep(100);
    }
  }

  const browser = {
    session: null,
    downloadDir,
    // 쉬고 있던 서비스 워커가 다운로드 이벤트로 다시 뜨는지 보려고 확장 서비스 워커를 끈다.
    async stopWorkers() {
      const { targetInfos } = await call("Target.getTargets");
      const workers = targetInfos.filter((t) => t.type === "service_worker");
      for (const worker of workers) {
        await call("Target.closeTarget", { targetId: worker.targetId });
      }
      for (let i = 0; i < 30; i += 1) {
        const { targetInfos: after } = await call("Target.getTargets");
        if (!after.some((t) => t.type === "service_worker")) return workers.length > 0;
        await sleep(100);
      }
      return false;
    },
    // 다운로드 주소를 새 탭으로 열고, 저장이 끝난 파일 이름을 돌려준다.
    async download(path) {
      const before = new Set(readdirSync(downloadDir));
      await call("Target.createTarget", { url: pageUrl + path.slice(1) });
      for (let i = 0; i < 50; i += 1) {
        const added = readdirSync(downloadDir).filter((name) => !before.has(name) && !name.endsWith(".crdownload"));
        if (added.length) return added[0];
        await sleep(100);
      }
      return null;
    },
    send: (method, params) => call(method, params, browser.session),
    // 시나리오마다 새 탭을 열어 서로 영향을 주지 않게 한다.
    async newPage(url) {
      const { targetId } = await call("Target.createTarget", { url });
      const { sessionId } = await call("Target.attachToTarget", { targetId, flatten: true });
      browser.session = sessionId;
      for (let i = 0; i < 50; i += 1) {
        const { result } = await browser.send("Runtime.evaluate", {
          expression: "location.href.startsWith('http') && document.readyState === 'complete'"
        });
        if (result.value) break;
        await sleep(100);
      }
    },
    close() {
      chrome.kill();
    }
  };
  return browser;
}

async function openPage(browser) {
  await browser.newPage(pageUrl);
}

async function setFiles(browser, expression, files) {
  const { result } = await browser.send("Runtime.evaluate", { expression });
  if (!result.objectId) {
    const { result: page } = await browser.send("Runtime.evaluate", { expression: "location.href + ' ' + document.readyState + ' ' + document.body?.innerHTML.length" });
    throw new Error(`요소를 찾지 못함: ${expression} → ${JSON.stringify(result)} / 페이지: ${page.value}`);
  }
  await browser.send("DOM.setFileInputFiles", { objectId: result.objectId, files });
}

async function dropFiles(browser, files, text) {
  const data = { items: text ? [{ mimeType: "text/plain", data: text }] : [], files, dragOperationsMask: 1 };
  for (const type of ["dragEnter", "dragOver", "drop"]) {
    await browser.send("Input.dispatchDragEvent", { type, x: 100, y: 100, data });
  }
}

async function waitFor(name) {
  for (let i = 0; i < 50; i += 1) {
    if (results.has(name)) {
      return results.get(name);
    }
    await sleep(100);
  }
  return null;
}

function sleep(ms) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

const checks = [];
function check(label, ok, detail) {
  checks.push(ok);
  console.log(`${ok ? "통과" : "실패"}  ${label}${detail ? `  (${detail})` : ""}`);
}

try {
  const plain = await launch(null);
  await openPage(plain);
  await setFiles(plain, "document.getElementById('js')", [nfdPath]);
  const baseline = await waitFor("js");
  check("0. 확장 없음: NFD로 전송되는 문제 재현", baseline?.names[0] && form(baseline.names[0]) === "NFD", baseline?.names.map(form).join(","));
  plain.close();
  results.clear();

  const browser = await launch(extensionDir);

  await openPage(browser);
  await setFiles(browser, "document.getElementById('js')", [nfdPath]);
  const js = await waitFor("js");
  check("1. JS 업로드(FormData)", js?.names[0] === nfdName.normalize("NFC"), js?.names.map(form).join(","));

  await openPage(browser);
  await setFiles(browser, "document.getElementById('formInput')", [nfdPath]);
  const formResult = await waitFor("form");
  check("2. 일반 폼 제출", formResult?.names[0] === nfdName.normalize("NFC"), formResult?.names.map(form).join(","));

  await openPage(browser);
  await dropFiles(browser, [nfdPath], "keep-me");
  const dropped = await waitFor("drop");
  check(
    "3. 드래그 앤 드롭, 함께 담긴 텍스트 유지",
    dropped?.names[0] === nfdName.normalize("NFC") && dropped?.text === "keep-me",
    `${dropped?.names.map(form).join(",")}, text=${dropped?.text}`
  );

  results.delete("drop");
  await openPage(browser);
  await dropFiles(browser, [nfcPath]);
  const untouched = await waitFor("drop");
  check(
    "4. 이미 NFC인 파일은 개입하지 않음",
    untouched?.names[0] === nfcName && untouched?.trusted === "true",
    `${untouched?.names.map(form).join(",")}, 원래 이벤트=${untouched?.trusted}`
  );

  await openPage(browser);
  await setFiles(browser, "document.querySelector('shadow-upload').shadowRoot.querySelector('input')", [nfdPath]);
  const shadow = await waitFor("shadow");
  check("5. Shadow DOM 안의 입력칸", shadow?.names[0] === nfdName.normalize("NFC"), shadow?.names.map(form).join(","));

  await openPage(browser);
  await setFiles(browser, "document.getElementById('folder')", [folderPath]);
  const folder = await waitFor("folder");
  // 폴더 업로드는 "폴더/파일" 형태의 상대 경로로 전송된다. 경로와 이름이 원래 형태 그대로인지 본다.
  check(
    "6. 폴더 업로드는 건드리지 않음",
    folder?.names[0] === `${"업로드폴더".normalize("NFD")}/${nfdName}`,
    `${folder?.names.map(form).join(",")}, 경로 유지=${folder?.names[0]?.includes("/")}`
  );

  const off = await browser.download("/download/legacy");
  check("7. 다운로드 기능 꺼짐(기본): 받은 이름을 건드리지 않음", off === downloads.legacy.name, off && form(off));
  browser.close();

  const granted = await launch(grantedDir);
  const legacy = await granted.download("/download/legacy");
  check("8. 다운로드 기능 켬: 옛날 방식 이름을 NFC로 저장", legacy === downloads.legacy.name.normalize("NFC"), legacy && form(legacy));
  const byPath = await granted.download(`/file/${encodeURIComponent(pathDownloadName)}`);
  check("9. 다운로드 기능 켬: 주소 경로 이름을 NFC로 저장", byPath === pathDownloadName.normalize("NFC"), byPath && form(byPath));
  const nfcDownload = await granted.download("/download/nfc");
  check("10. 다운로드 기능 켬: 이미 NFC인 이름은 그대로", nfcDownload === downloads.nfc.name, nfcDownload && form(nfcDownload));
  const again = await granted.download("/download/legacy");
  check("11. 다운로드 기능 켬: 같은 이름이 있으면 덮어쓰지 않음", again && again !== legacy && again === again.normalize("NFC"), again);
  const stopped = await granted.stopWorkers();
  const woke = await granted.download(`/file/${encodeURIComponent("바_다시깨움.txt".normalize("NFD"))}`);
  check("12. 서비스 워커가 꺼져 있어도 다운로드 때 다시 떠서 NFC로 저장", stopped && woke === woke?.normalize("NFC"), `워커 종료=${stopped}, ${woke && form(woke)}`);
  granted.close();
} finally {
  server.close();
  await sleep(300);
  rmSync(workDir, { recursive: true, force: true });
}

const failed = checks.filter((ok) => !ok).length;
console.log(failed === 0 ? `\n전체 통과 (${checks.length}개)` : `\n실패 ${failed}개 / ${checks.length}개`);
process.exit(failed === 0 ? 0 : 1);

const $ = (id) => document.getElementById(id);
function directDownloadUrl(raw) {
  try {
    const u = new URL(raw);
    let m = u.pathname.match(/\/document\/d\/([^/]+)/);
    if (m) return `https://docs.google.com/document/d/${m[1]}/export?format=docx`;
    m = u.pathname.match(/\/spreadsheets\/d\/([^/]+)/);
    if (m) return `https://docs.google.com/spreadsheets/d/${m[1]}/export?format=xlsx`;
    m = u.pathname.match(/\/presentation\/d\/([^/]+)/);
    if (m) return `https://docs.google.com/presentation/d/${m[1]}/export/pptx`;
    m = u.pathname.match(/\/file\/d\/([^/]+)/) || u.search.match(/[?&]id=([^&]+)/);
    if (m && /drive\.google\.com$/i.test(u.hostname)) return `https://drive.usercontent.google.com/download?id=${m[1]}&export=download&confirm=t`;
  } catch { /* keep original URL */ }
  return raw;
}
function cleanFilename(title, rawUrl) {
  let name = (title || "classroom-file").replace(/\s+/g, " ").trim();
  name = name.replace(/(Microsoft[- ]Word|Microsoft[- ]Excel|Microsoft[- ]PowerPoint|Brave HTML Document|PDF File|PDF|DOCX?|XLSX?|PPTX?)$/i, "").trim();
  let ext = "";
  try {
    const u = new URL(directDownloadUrl(rawUrl));
    ext = u.searchParams.get("format") || (u.pathname.match(/\.(pdf|docx?|xlsx?|pptx?)(?:$|\?)/i)?.[1] || "");
  } catch { /* use the title extension */ }
  if (!ext) ext = name.match(/\.(pdf|docx?|xlsx?|pptx?)$/i)?.[1] || "";
  if (ext && !name.toLowerCase().endsWith(`.${ext.toLowerCase()}`)) name += `.${ext}`;
  return name.replace(/[^a-z0-9._-]+/gi, "-").replace(/-+/g, "-").slice(0, 100) || `classroom-file${ext ? `.${ext}` : ""}`;
}
async function waitForDownload(id) {
  for (let i = 0; i < 120; i++) {
    const [item] = await chrome.downloads.search({ id });
    if (item?.state === "complete") return item.filename;
    if (item?.state === "interrupted") throw new Error("download interrupted");
    await new Promise(r => setTimeout(r, 500));
  }
  throw new Error("download timed out");
}
async function importIntoCortex(path, item, bundle) {
  // `file` is only the extension's display category; Cortex detects the real
  // source kind from the downloaded filename (pdf/docx/xlsx/pptx/etc.).
  const response = await fetch("http://127.0.0.1:47821/api/classroom/import", { method: "POST", headers: { "Content-Type": "application/json", "X-Cortex-Bridge": "cortex-local" }, body: JSON.stringify({ subject: bundle.subject, topic: bundle.topic, path, name: item.title }) });
  if (!response.ok) throw new Error((await response.text()) || "Cortex import failed");
}
const fallback = { Biology: ["Cell biology / Membranes", "Genetics"], Mathematics: ["Algebra", "Geometry"] };
let structure = fallback;
const syncButton = document.createElement("button");
syncButton.textContent = "Sync subjects from Cortex";
syncButton.className = "mini";
syncButton.style.marginBottom = "12px";
document.querySelector(".card").prepend(syncButton);
syncButton.onclick = async () => {
  syncButton.disabled = true;
  try {
    const response = await fetch("http://127.0.0.1:47821/api/classroom/structure", { headers: { "X-Cortex-Bridge": "cortex-local" } });
    if (!response.ok) throw new Error("Cortex bridge unavailable");
    const data = await response.json();
    structure = Object.fromEntries((data.subjects || []).map(s => [s.name, (s.topics || []).map(t => t.name)]));
    await chrome.storage.local.set({ cortexStructure: structure });
    renderChoices();
    $("status").className = "status ok";
    $("status").textContent = `Synced ${Object.keys(structure).length} subjects from Cortex.`;
  } catch (e) { $("status").className = "status err"; $("status").textContent = "Open Cortex first, then try Sync again."; }
  finally { syncButton.disabled = false; }
};
function renderChoices() {
  const previousSubject = $("subject").value;
  const previousTopic = $("topic").value;
  const subjects = Object.keys(structure);
  $("subject").innerHTML = subjects.map(s => `<option>${s}</option>`).join("") || '<option value="">Add a subject</option>';
  if (subjects.includes(previousSubject)) $("subject").value = previousSubject;
  const topics = structure[$("subject").value] || [];
  $("topic").innerHTML = topics.map(t => `<option>${t}</option>`).join("") || '<option value="">Add a topic</option>';
  if (topics.includes(previousTopic)) $("topic").value = previousTopic;
  const full = $("topic").value;
  const parts = full.split("/").map(x => x.trim());
  $("subtopic").innerHTML = (parts[1] ? `<option>${parts[1]}</option>` : '<option value="">No subtopic</option>');
}
chrome.storage.local.get({ cortexStructure: fallback }, (data) => { structure = data.cortexStructure || fallback; renderChoices(); });
$("subject").onchange = renderChoices;
$("topic").onchange = renderChoices;
for (const [id, message] of [["addSubject", "Subject name"], ["addTopic", "Topic or subtopic name"], ["addSubtopic", "Subtopic name"]]) $(id).onclick = () => { const name = prompt(message); if (!name?.trim()) return; const subject = $("subject").value || name.trim(); if (id === "addSubject") structure[name.trim()] = []; else { if (!structure[subject]) structure[subject] = []; structure[subject].push(id === "addSubtopic" && $("topic").value ? `${$("topic").value.split("/")[0].trim()} / ${name.trim()}` : name.trim()); } chrome.storage.local.set({ cortexStructure: structure }, renderChoices); };
$("capture").onclick = async () => {
  const button = $("capture");
  button.disabled = true;
  $("status").className = "status";
  $("status").textContent = "Reading the active Classroom page…";
  try {
    const [tab] = await chrome.tabs.query({ active: true, currentWindow: true });
    const result = await chrome.scripting.executeScript({ target: { tabId: tab.id }, func: () => {
      const items = [...document.querySelectorAll('a[href]')].map(a => ({ title: (a.textContent || a.getAttribute('aria-label') || a.href).trim().replace(/\s+/g, ' '), url: a.href, kind: /youtube\.com|youtu\.be/i.test(a.href) ? 'youtube' : 'file' })).filter(x => x.url.startsWith('http') && !x.url.includes('accounts.google.com') && /youtube\.com|youtu\.be|docs\.google\.com|drive\.google\.com|\.(pdf|doc|docx|xls|xlsx|ppt|pptx)(?:[?#]|$)/i.test(x.url));
      return { pageTitle: document.title, pageUrl: location.href, items: [...new Map(items.map(x => [x.url, x])).values()] };
    }});
    const data = result[0].result;
    const bundle = { format: "cortex-classroom-capture", version: 1, capturedAt: new Date().toISOString(), subject: $("subject").value.trim(), topic: $("topic").value.trim(), subtopic: $("subtopic").value.trim(), ...data };
    const files = bundle.items.filter(item => item.kind === "file");
    for (const item of files) {
      const safe = cleanFilename(item.title, item.url);
      const downloadId = await chrome.downloads.download({ url: directDownloadUrl(item.url), filename: `Cortex Classroom/${bundle.subject || "Subject"}/${bundle.topic || "Topic"}/${safe}`, saveAs: false });
      const path = await waitForDownload(downloadId);
      await importIntoCortex(path, item, bundle);
    }
    const blob = new Blob([JSON.stringify(bundle, null, 2)], { type: "application/json" });
    const url = URL.createObjectURL(blob);
    await chrome.downloads.download({ url, filename: `cortex-${(bundle.subject || "classroom").replace(/[^a-z0-9]+/gi, "-")}.cortex.json`, saveAs: true });
    $("status").className = "status ok";
    $("status").textContent = `Downloaded ${files.length} files and saved ${bundle.items.length - files.length} YouTube links. JSON backup is ready.`;
  } catch (e) { $("status").className = "status err"; $("status").textContent = `Capture failed: ${e.message}`; }
  finally { button.disabled = false; }
};

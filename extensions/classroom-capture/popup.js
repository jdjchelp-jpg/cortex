const $ = (id) => document.getElementById(id);
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
  const subjects = Object.keys(structure);
  $("subject").innerHTML = subjects.map(s => `<option>${s}</option>`).join("") || '<option value="">Add a subject</option>';
  const topics = structure[$("subject").value] || [];
  $("topic").innerHTML = topics.map(t => `<option>${t}</option>`).join("") || '<option value="">Add a topic</option>';
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
      const items = [...document.querySelectorAll('a[href]')].map(a => ({ title: (a.textContent || a.getAttribute('aria-label') || a.href).trim().replace(/\s+/g, ' '), url: a.href, kind: /drive|docs|slides|sheets|pdf|download/i.test(a.href) ? 'file' : 'link' })).filter(x => x.url.startsWith('http') && !x.url.includes('accounts.google.com'));
      return { pageTitle: document.title, pageUrl: location.href, items: [...new Map(items.map(x => [x.url, x])).values()] };
    }});
    const data = result[0].result;
    const bundle = { format: "cortex-classroom-capture", version: 1, capturedAt: new Date().toISOString(), subject: $("subject").value.trim(), topic: $("topic").value.trim(), subtopic: $("subtopic").value.trim(), ...data };
    const blob = new Blob([JSON.stringify(bundle, null, 2)], { type: "application/json" });
    const url = URL.createObjectURL(blob);
    await chrome.downloads.download({ url, filename: `cortex-${(bundle.subject || "classroom").replace(/[^a-z0-9]+/gi, "-")}.cortex.json`, saveAs: true });
    $("status").className = "status ok";
    $("status").textContent = `Captured ${bundle.items.length} links/files. Your Cortex bundle is ready.`;
  } catch (e) { $("status").className = "status err"; $("status").textContent = `Capture failed: ${e.message}`; }
  finally { button.disabled = false; }
};

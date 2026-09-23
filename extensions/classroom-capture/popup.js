const $ = (id) => document.getElementById(id);
$("capture").onclick = async () => {
  $("status").textContent = "Collecting…";
  try {
    const [tab] = await chrome.tabs.query({ active: true, currentWindow: true });
    const result = await chrome.scripting.executeScript({ target: { tabId: tab.id }, func: () => {
      const items = [...document.querySelectorAll('a[href]')].map(a => ({ title: (a.textContent || a.getAttribute('aria-label') || a.href).trim().replace(/\s+/g, ' '), url: a.href, kind: /drive|docs|slides|sheets|pdf|download/i.test(a.href) ? 'file' : 'link' })).filter(x => x.url.startsWith('http') && !x.url.includes('accounts.google.com'));
      return { pageTitle: document.title, pageUrl: location.href, items: [...new Map(items.map(x => [x.url, x])).values()] };
    }});
    const data = result[0].result;
    const bundle = { format: "cortex-classroom-capture", version: 1, capturedAt: new Date().toISOString(), subject: $("subject").value.trim(), topic: $("topic").value.trim(), ...data };
    const blob = new Blob([JSON.stringify(bundle, null, 2)], { type: "application/json" });
    const url = URL.createObjectURL(blob);
    await chrome.downloads.download({ url, filename: `cortex-${(bundle.subject || "classroom").replace(/[^a-z0-9]+/gi, "-")}.cortex.json`, saveAs: true });
    $("status").textContent = `Captured ${bundle.items.length} links/files. Import the .cortex.json bundle into Cortex.`;
  } catch (e) { $("status").textContent = `Capture failed: ${e.message}`; }
};

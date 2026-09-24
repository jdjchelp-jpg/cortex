function cortexCollectClassroomItems() {
  const items = [...document.querySelectorAll('a[href]')].map((a) => ({
    title: (a.textContent || a.getAttribute('aria-label') || a.href).trim().replace(/\s+/g, ' '),
    url: a.href,
    kind: /youtube\.com|youtu\.be/i.test(a.href) ? 'youtube' : 'file'
  })).filter((x) => {
    if (!x.url.startsWith('http') || x.url.includes('accounts.google.com')) return false;
    return /youtube\.com|youtu\.be|docs\.google\.com|drive\.google\.com|\.(pdf|doc|docx|xls|xlsx|ppt|pptx)(?:[?#]|$)/i.test(x.url);
  });
  return { pageTitle: document.title, pageUrl: location.href, items: [...new Map(items.map((x) => [x.url, x])).values()] };
}

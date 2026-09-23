function cortexCollectClassroomItems() {
  const items = [...document.querySelectorAll('a[href]')].map((a) => ({
    title: (a.textContent || a.getAttribute('aria-label') || a.href).trim().replace(/\s+/g, ' '),
    url: a.href,
    kind: /drive|docs|slides|sheets|pdf|download/i.test(a.href) ? 'file' : 'link'
  })).filter((x) => x.url.startsWith('http') && !x.url.includes('accounts.google.com'));
  return { pageTitle: document.title, pageUrl: location.href, items: [...new Map(items.map((x) => [x.url, x])).values()] };
}

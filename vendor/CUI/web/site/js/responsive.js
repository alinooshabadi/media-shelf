// Responsive behaviour for the site chrome: below 860 px the sidebar is a
// drawer behind a menu button in the top bar. Closes on navigation, on the
// scrim, and with Escape. Kept apart from app.js on purpose.

const MENU = '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round"><path d="M4 7h16M4 12h16M4 17h16"/></svg>';

function setup() {
  const bar = document.querySelector('.topbar');
  const sidebar = document.getElementById('sidebar');
  if (!bar || !sidebar || bar.querySelector('.nav-toggle')) return;
  const btn = document.createElement('button');
  btn.className = 'icon-btn nav-toggle';
  btn.setAttribute('aria-label', 'Open navigation');
  btn.setAttribute('aria-controls', 'sidebar');
  btn.setAttribute('aria-expanded', 'false');
  btn.innerHTML = MENU;
  bar.prepend(btn);
  let scrim = null;
  const close = () => {
    if (!document.body.classList.contains('nav-open')) return;
    document.body.classList.remove('nav-open');
    btn.setAttribute('aria-expanded', 'false');
    scrim?.remove(); scrim = null;
  };
  const open = () => {
    document.body.classList.add('nav-open');
    btn.setAttribute('aria-expanded', 'true');
    scrim = document.createElement('div');
    scrim.className = 'nav-scrim';
    scrim.addEventListener('click', close);
    document.body.append(scrim);
    sidebar.querySelector('[aria-current="page"]')?.scrollIntoView({ block: 'center' });
  };
  btn.addEventListener('click', () => (document.body.classList.contains('nav-open') ? close() : open()));
  window.addEventListener('hashchange', close);
  document.addEventListener('keydown', (e) => { if (e.key === 'Escape') close(); });
  matchMedia('(min-width: 861px)').addEventListener('change', (e) => { if (e.matches) close(); });
}

if (document.readyState === 'loading') document.addEventListener('DOMContentLoaded', setup);
else setup();

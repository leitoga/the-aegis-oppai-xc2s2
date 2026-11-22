// SPDX-License-Identifier: GPL-3.0-or-later
// XC2 Launcher: one page (main), the first run / "build another version" page (setup), the build progress page (build),
// and sheets over them (settings, release notes, saves, vendor license).
import { t, setLang, lang, err, fmtDate, fmtSize, LANGS } from './i18n.js';
import { icon } from './icons.js';
import { mockInvoke, mockListen } from './mock.js';

const T = window.__TAURI__;
const invoke = T ? T.core.invoke : mockInvoke;
const listen = T ? T.event.listen : mockListen;
const win = T ? T.window.getCurrentWindow() : null;
if (!T) document.documentElement.classList.add('browser');

const $ = (s, r = document) => r.querySelector(s);
const esc = (s) => String(s ?? '').replace(/[&<>"']/g, (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' }[c]));
const base = (p) => String(p).split('/').filter(Boolean).pop() || p;

const S = {
  st: null,            // get_state
  releases: [],        // the port's releases
  launcherRel: null,   // a newer launcher, if any
  relError: null,
  checking: false,
  view: 'main',        // main | setup | build
  sel: null,           // the instance shown
  sizes: {},
  saves: {},
  covers: {},
  opts: {},
  wineDl: null,
  hw: null,
  log: [],
  showLog: false,
  build: null,         // build progress
  wiz: { source: '', check: null, tag: '', zip: '', folder: '', mode: 'new' },
  vendor: null,        // {id, phase, done, total}
};

const GAME_LANGS = [
  ['LatinAmericanSpanish', 'Español (Latinoamérica)', 'Spanish (Latin America)'],
  ['Spanish', 'Español (España)', 'Spanish (Spain)'],
  ['AmericanEnglish', 'English (US)', 'English (US)'],
  ['BritishEnglish', 'English (UK)', 'English (UK)'],
  ['French', 'Français', 'French'],
  ['German', 'Deutsch', 'German'],
  ['Italian', 'Italiano', 'Italian'],
  ['Japanese', '日本語', 'Japanese'],
  ['Korean', '한국어', 'Korean'],
  ['SimplifiedChinese', '简体中文', 'Chinese (Simplified)'],
  ['TraditionalChinese', '繁體中文', 'Chinese (Traditional)'],
];
const gameLangName = (code) => { const g = GAME_LANGS.find((x) => x[0] === code); return g ? (lang() === 'es' ? g[1] : g[2]) : code; };

// ---------- data ----------
function cfg() { return S.st?.config || {}; }
function instances() { return S.st?.instances || []; }
function current() { return instances().find((i) => i.path === S.sel) || instances().find((i) => i.path === cfg().default) || instances()[0]; }
function channelReleases() {
  const pre = cfg().channel === 'preliminar';
  return S.releases.filter((r) => r.asset_url && (pre || !r.prerelease));
}
function newerThan(a, b) {
  const p = (s) => String(s || '').replace(/^v/, '').split(/[.-]/).map((x) => parseInt(x) || 0);
  const x = p(a), y = p(b);
  for (let i = 0; i < Math.max(x.length, y.length); i++) { if ((x[i] || 0) !== (y[i] || 0)) return (x[i] || 0) > (y[i] || 0); }
  return false;
}
function updateFor(inst) {
  if (!inst || !inst.version) return null;
  const r = channelReleases()[0];
  return r && newerThan(r.version, inst.version) ? r : null;
}

async function refresh() {
  S.st = await invoke('get_state');
  // English unless the player chose another language (⚙ → General)
  setLang(cfg().lang || 'en');
  if (!S.hw) invoke('hardware').then((h) => { S.hw = h; render(); }).catch(() => {});
  if (!S.sel || !instances().some((i) => i.path === S.sel)) S.sel = cfg().default || instances()[0]?.path || null;
  if (S.st.building?.state === 'running') { S.build = S.st.building; S.view = 'build'; if (!S.log.length) S.log = await invoke('build_log'); }
  else if (!instances().length && S.view === 'main') S.view = 'setup';
  $('#app-ver').textContent = 'v' + S.st.version;
  for (const i of instances()) {
    if (!(i.path in S.sizes)) { S.sizes[i.path] = null; invoke('instance_size', { path: i.path }).then((n) => { S.sizes[i.path] = n; render(); }); }
    invoke('saves', { path: i.path }).then((s) => { S.saves[i.path] = s; render(); });
    invoke('read_options', { path: i.path }).then((o) => { S.opts[i.path] = o; render(); });
  }
  render();
}

async function loadReleases(refreshNow = false) {
  S.checking = true; render();
  try {
    const r = await invoke('releases', { refresh: refreshNow });
    S.releases = r.game || []; S.launcherRel = r.launcher || null; S.relError = r.error || null;
    if (refreshNow) toast(S.relError ? t('offline', { e: S.relError }) : (updateFor(current()) || S.launcherRel ? t('updateAvailable') : t('upToDate')), !!S.relError);
  } catch (e) { S.relError = String(e); }
  S.checking = false; render();
}

// ---------- rendering ----------
let lastHtml = "";
function render() {
  if (!S.st) return;
  const v = $('#view');
  const inst = current();
  const bd = $('#backdrop');
  const hide = S.view === 'build';
  if (bd.hidden !== hide) bd.hidden = hide;
  const img = bd.querySelector('img');
  if (!img.getAttribute('src')) img.src = 'logo-bg.png';
  const html = S.view === 'setup' ? setupView() : S.view === 'build' ? buildView() : mainView();
  // the page is rebuilt only when it changed: data that arrives often (game state, sizes, saves) must not make it flicker
  if (html === lastHtml) return;
  lastHtml = html;
  const active = document.activeElement?.dataset?.change || document.activeElement?.dataset?.input;
  v.innerHTML = html;
  if (active) v.querySelector(`[data-change="${active}"],[data-input="${active}"]`)?.focus();
  const log = $('.log');
  if (log) log.scrollTop = log.scrollHeight;
  $('#btn-settings').title = t('settings'); $('#btn-min').title = t('minimize'); $('#btn-max').title = document.documentElement.classList.contains('maximized') ? t('restoreWin') : t('maximizeWin'); $('#btn-close').title = t('close');
}

// ---------- what the game needs besides the launcher ----------
// No Wine at all: a card that downloads the launcher's portable Wine 11.22 (no password; Set up downloads it by itself too).
function depsCard() {
  if (!S.st || S.st.runners.wine) return '';
  const p = S.wineDl;
  return `<div class="banner deps"><div class="dot warn">${icon('alert')}</div>
    <div class="txt"><b>${t('wineMissing')}</b><span>${t('wineMissingBody')}</span>
      ${p ? `<div class="bar" style="margin-top:8px"><i style="width:${p.total ? (p.done / p.total * 100).toFixed(1) : 100}%"></i></div><span class="deps-line">${esc(p.msg || '')}</span>` : ''}</div>
    <div class="acts"><button class="btn accent-outline" data-act="wine-install" ${p ? 'disabled' : ''}>${p ? '<span class="spin"></span>' : icon('download')}${t('installWine')}</button></div></div>`;
}

// One notice for everything there is to update, only when there is something: the launcher (the program) and/or the game
// (the port's package of the selected install), each named for what it is and what it takes.
function banner() {
  const inst = current();
  const cfgv = cfg();
  const lr = S.launcherRel && cfgv.dismissed !== 'L' + S.launcherRel.version ? S.launcherRel : null;
  const up = updateFor(inst);
  const gr = up && cfgv.dismissed !== up.tag ? up : null;
  if (!lr && !gr) return '';
  const row = (what, from, to, hint, notes, act, dismiss) => `<div class="up-row">
      <div class="txt"><b>${what} <span class="ver">${esc(from)} → ${esc(to)}</span></b><span>${hint}</span></div>
      <div class="acts">${notes}<button class="btn ghost sm" data-act="dismiss" data-which="${esc(dismiss)}">${t('later')}</button>
      <button class="btn accent-outline" data-act="${act}">${t('update')}</button></div></div>`;
  return `<div class="banner updates"><div class="dot">${icon('up')}</div><div class="up-list">
    <b class="up-title">${t('updatesTitle')}</b>
    ${lr ? row(t('rowLauncher'), 'v' + S.st.version, 'v' + lr.version, t('hintLauncher') + (gr ? ' ' + t('updateFirst') : ''),
      `<button class="btn ghost sm" data-act="notes" data-which="launcher">${t('whatsNew')}</button>`, 'update-launcher', 'L' + lr.version) : ''}
    ${gr ? row(t('rowGame'), inst.tag || inst.version, gr.tag, t('hintGame'),
      `<button class="btn ghost sm" data-act="notes" data-which="${esc(gr.tag)}">${t('whatsNew')}</button>`, 'update', gr.tag) : ''}
  </div></div>`;
}

function instTitle(i) { return `${i.tag || (i.version ? 'v' + i.version : '?')} · ${esc(base(i.path))}`; }

function features(i) {
  const f = [gameLangName(i.language)];
  if (i.dlss) f.push('NVIDIA DLSS');
  if (i.fsr) f.push('AMD FSR 3.1');
  return f.join(', ');
}

function mainView() {
  const inst = current();
  const c = cfg();
  const up = updateFor(inst);
  const running = S.st.running;
  const cover = inst && S.covers[inst.path];
  const saves = inst && S.saves[inst.path];
  const isDefault = inst && inst.path === c.default;
  const latest = channelReleases()[0];

  const hero = `
    <div class="hero">
      <img class="emblem" src="logo.png" alt="">
      <h1>${t('gameTitle')}</h1>
      <div class="game-sub">${t('gameSub', { version: esc(inst?.tag || '') })}</div>
      <div class="release-pill">${inst && !up && latest ? `<span class="muted">${esc(latest.tag)} · ${esc(fmtDate(Date.parse(latest.date) / 1000))}</span><span class="link" data-act="notes" data-which="${esc(latest.tag)}">${t('whatsNew')}</span>` : ''}</div>
    </div>`;

  const left = !inst ? '' : `
    <div class="label">${t('channel')}</div>
    <div class="select"><select data-change="channel">
      <option value="estable" ${c.channel !== 'preliminar' ? 'selected' : ''}>${t('stable')}</option>
      <option value="preliminar" ${c.channel === 'preliminar' ? 'selected' : ''}>${t('preview')}</option>
    </select></div>
    <div class="label">${t('version')}</div>
    <div class="select"><select data-change="select-instance">
      ${instances().map((i) => `<option value="${esc(i.path)}" ${i.path === inst.path ? 'selected' : ''}>${i.path === c.default ? '✓ ' : ''}${instTitle(i)}</option>`).join('')}
    </select></div>
    <div class="play-row ${up && inst.ready ? '' : 'single'}">
      ${inst.ready
        ? (running ? `<button class="btn primary big" data-act="stop">${icon('stop')}${t('stop')}</button>`
                   : `<button class="btn primary big" data-act="play">${icon('play')}${t('play')}</button>`)
        : `<button class="btn primary big" data-act="continue">${icon('hammer')}${t('continueSetup')}</button>`}
      ${up && inst.ready ? `<button class="btn big emerald" data-act="update">${icon('up')}${t('updateGame')}</button>` : ''}
    </div>
    <div class="note">${inst.ready ? `${isDefault ? t('defaultNote') : `${t('notDefaultNote')} <span class="link" data-act="make-default" data-path="${esc(inst.path)}">${t('makeDefault')}</span>.`}
      ${inst.built ? t('builtOn', { date: fmtDate(inst.built) }) + ', ' : ''}${esc(features(inst))}.` : t('notReady')}</div>
    <button class="remove" data-act="remove" data-path="${esc(inst.path)}">${icon('trash')}${t('remove')}</button>
    <div class="cards">
      <button class="card" data-act="saves">
        <div class="ic">${icon('save')}</div>
        <div class="txt"><b>${t('savesTitle')}</b><span>${saves && saves.files.length ? esc(t('savesSub', { n: saves.files.length, date: fmtDate(saves.latest) })) : t('savesNone')}</span></div>
        ${icon('chev', 'chev')}
      </button>
      <button class="card" data-act="vendor" data-id="dlss">
        <div class="ic ${inst.dlss ? 'on' : ''}">${icon('chip')}</div>
        <div class="txt"><b>${t('dlssTitle')}</b><span>${inst.dlss ? t('dlssOn', { v: 'v2.14.16' }) : t('dlssOff')}</span></div>
        ${icon('chev', 'chev')}
      </button>
      <button class="card" data-act="vendor" data-id="fsr">
        <div class="ic ${inst.fsr ? 'on' : ''}">${icon('bolt')}</div>
        <div class="txt"><b>${t('fsrTitle')}</b><span>${inst.fsr ? t('fsrOn', { v: 'v2.4.0' }) : t('fsrOff')}</span></div>
        ${icon('chev', 'chev')}
      </button>
    </div>`;

  const total = instances().reduce((a, i) => a + (S.sizes[i.path] || 0), 0);
  const measuring = instances().some((i) => S.sizes[i.path] == null);
  const usage = measuring ? t('measuring') : instances().length === 1
    ? t('usage', { n: 1, size: fmtSize(total), dir: esc(instances()[0].display.replace(/\/[^/]*$/, '') || '~') })
    : t('usagePlural', { n: instances().length, size: fmtSize(total) });

  const right = `
    <div class="inst-head">
      <div class="label">${t('instances')}</div>
      <button class="act" data-act="add-folder">${icon('folder')}${t('addFolder')}</button>
      <button class="act" data-act="build-other">${icon('plus')}${t('buildOther')}</button>
    </div>
    <div class="list">${instances().map((i) => {
      const u = updateFor(i);
      const size = S.sizes[i.path];
      return `<div class="row ${i.path === inst?.path ? 'sel' : ''}" title="${esc(i.path)}" data-act="select" data-path="${esc(i.path)}">
        <button class="star ${i.path === c.default ? 'on' : ''}" data-act="make-default" data-path="${esc(i.path)}" title="${t('setDefault')}">${icon(i.path === c.default ? 'star' : 'starO')}</button>
        <div class="txt"><b>${instTitle(i)}</b><span>${i.exists ? (i.ready ? t('builtOn', { date: fmtDate(i.built) }) : t('notBuilt')) + (size ? ', ' + fmtSize(size) : '') : t('missing')}</span></div>
        <div class="badges">
          ${u ? `<span class="badge solid">${t('updateAvailable')}</span>` : ''}
          ${i.path === c.default ? `<span class="badge emerald">${t('defaultBadge')}</span>` : ''}
          ${!i.ready ? `<span class="badge warn">${t('notBuilt')}</span>` : ''}
          <span class="badge accent">${i.channel === 'preliminar' ? t('preview').toLowerCase() : t('stable').toLowerCase()}</span>
        </div>
        <button class="more" data-act="menu" data-path="${esc(i.path)}">${icon('dots')}</button>
      </div>`;
    }).join('')}</div>
    <div class="inst-foot"><span>${usage}</span>
      <button class="act" data-act="check-updates">${S.checking ? '<span class="spin"></span>' : icon('refresh')}${S.checking ? t('checking') : t('checkUpdates')}</button></div>
    ${inst ? gameSettings(inst) : ''}`;

  return `<div class="page">${depsCard()}${banner()}${hero}<div class="grid"><div class="col">${left}</div><div class="col">${right}</div></div></div>`;
}

function setupView() {
  const w = S.wiz;
  const rels = channelReleases();
  if (!w.tag && !w.zip && rels[0]) w.tag = rels[0].tag;
  const ver = w.zip ? '' : (rels.find((r) => r.tag === w.tag)?.version || '');
  if (!w.folder) invoke('suggest_folder', { version: ver }).then((f) => { if (!S.wiz.folder) { S.wiz.folder = f; render(); } });
  const srcOk = w.check?.ok;
  const canGo = srcOk && (w.tag || w.zip) && w.folder;
  return `<div class="setup">
    <div class="hero-space" style="height:40px"></div>
    <img class="emblem" src="logo.png" alt="">
    <h1>${instances().length ? t('buildOther') : t('welcome')}</h1>
    <p class="lead">${t('welcomeBody')}</p>
    <div class="step ${srcOk ? 'ok' : ''}"><div class="num">${srcOk ? icon('check') : 1}</div><div class="sb">
      <b>${t('stepFiles')}</b><span>${t('stepFilesBody')}</span>
      <div class="acts"><button class="btn" data-act="pick-source" data-kind="nup">${icon('file')}${t('pickNup')}</button>
      <button class="btn" data-act="pick-source" data-kind="folder">${icon('folder')}${t('pickFolder')}</button></div>
      ${w.source ? `<div class="path">${esc(w.source)}<br>${w.check ? (w.check.ok ? `<span class="ok">✓ ${esc(t('srcOk', { kind: t('kind_' + w.check.kind), detail: w.check.detail }))}</span>` : `<span class="bad">${t('srcBad')}</span>`) : ''}</div>` : ''}
    </div></div>
    <div class="step ${w.tag || w.zip ? 'ok' : ''}"><div class="num">${w.tag || w.zip ? icon('check') : 2}</div><div class="sb">
      <b>${t('stepVersion')}</b><span>xc2-ns2-recomp · github.com/weile10000/xc2-ns2-recomp</span>
      <div class="acts">
        <div class="mini-select"><select data-change="wiz-tag">
          ${rels.map((r) => `<option value="${esc(r.tag)}" ${r.tag === w.tag && !w.zip ? 'selected' : ''}>${esc(r.tag)}${r.prerelease ? ' · ' + t('preview') : ''} · ${fmtDate(Date.parse(r.date) / 1000)}</option>`).join('') || `<option>${t('noReleases')}</option>`}
          ${w.zip ? `<option selected>${esc(t('localZip', { name: base(w.zip) }))}</option>` : ''}
        </select></div>
        <button class="btn ghost" data-act="pick-zip">${t('useZip')}</button>
      </div>
    </div></div>
    <div class="step ${w.folder ? 'ok' : ''}"><div class="num">${w.folder ? icon('check') : 3}</div><div class="sb">
      <b>${t('stepFolder')}</b>
      <div class="acts" style="margin-top:10px"><input type="text" value="${esc(w.folder)}" data-input="wiz-folder" spellcheck="false"><button class="btn" data-act="pick-folder">${t('change')}</button></div>
    </div></div>
    <div style="display:flex;align-items:center;gap:16px;margin-top:22px">
      <button class="btn primary big" data-act="start-build" ${canGo ? '' : 'disabled'} style="min-width:220px">${icon('hammer')}${t('setUp')}</button>
      <span style="color:var(--text-3);font-size:13px;flex:1">${t('setUpHint')}</span>
      ${instances().length ? `<button class="btn ghost" data-act="back">${t('back')}</button>` : ''}
    </div>
    ${instances().length ? '' : `<div class="alt"><div class="txt"><b>${t('haveInstall')}</b><span>${t('haveInstallBody')}</span></div>
      <button class="btn" data-act="add-folder">${icon('folder')}${t('addFolder')}</button></div>`}
  </div>`;
}

function buildView() {
  const b = S.build || { state: 'running', step_index: 0 };
  const steps = ['package', 'prefix', 'extract', 'lift', 'zig', 'link', 'shaders', 'done'];
  const title = b.state === 'done' ? t('buildDone') : b.state === 'error' ? t('buildError') : b.state === 'cancelled' ? t('buildCancelled') : t('buildTitle', { version: S.wiz.tag || base(b.instance || '') });
  const frac = b.fraction >= 0 ? b.fraction : null;
  return `<div class="setup">
    <div class="hero-space" style="height:40px"></div>
    <h1>${esc(title)}</h1>
    <p class="lead">${esc(b.instance || '')}</p>
    ${b.state === 'running' ? `<div class="bar ${frac == null ? 'indet' : ''}"><i style="width:${frac == null ? '' : (frac * 100).toFixed(1) + '%'}"></i></div>` : ''}
    <div class="steps-list">${steps.map((s, i) => {
      let cls = '';
      if (b.state === 'done' || i < b.step_index) cls = 'done';
      else if (i === b.step_index) cls = b.state === 'running' ? 'now' : b.state === 'error' ? 'fail' : '';
      return `<div class="sl ${cls}"><div class="st">${cls === 'done' ? icon('check') : cls === 'fail' ? icon('x') : ''}</div>${t('step_' + s)}
        ${i === b.step_index && b.state === 'running' && b.message ? `<span class="sm">${esc(b.message)}</span>` : ''}</div>`;
    }).join('')}</div>
    ${b.state === 'error' ? `<p style="color:var(--danger)">${esc(b.message)}</p>` : ''}
    <div style="display:flex;gap:12px;align-items:center">
      ${b.state === 'running' ? `<button class="btn" data-act="cancel-build">${t('cancel')}</button>` : ''}
      ${b.state === 'done' ? `<button class="btn primary big" data-act="build-finished">${icon('play')}${t('done')}</button>` : ''}
      ${b.state === 'error' || b.state === 'cancelled' ? `<button class="btn primary" data-act="retry-build">${t('retry')}</button><button class="btn ghost" data-act="build-finished">${t('back')}</button>` : ''}
      <span style="flex:1"></span>
      <button class="btn ghost sm" data-act="toggle-log">${S.showLog ? t('hideLog') : t('showLog')}</button>
    </div>
    ${S.showLog ? `<div class="log">${esc(S.log.slice(-600).join('\n'))}</div>` : ''}
  </div>`;
}

// ---------- sheets ----------
function sheet(html, cls = '', key = '') {
  const prev = $('.overlay');
  if (key && prev && prev.dataset.key === key) {
    // the same sheet again (a tab or a value changed): new contents, no animation, scroll kept
    const sh = prev.querySelector('.sheet');
    const top = sh.querySelector('.body')?.scrollTop || 0;
    sh.className = `sheet ${cls}`;
    sh.style.animation = 'none';
    sh.innerHTML = html;
    const b = sh.querySelector('.body');
    if (b) b.scrollTop = top;
    return prev;
  }
  closeSheet();
  const o = document.createElement('div');
  o.className = 'overlay';
  o.dataset.key = key;
  o.innerHTML = `<div class="sheet ${cls}">${html}</div>`;
  o.addEventListener('mousedown', (e) => { if (e.target === o) closeSheet(); });
  document.body.appendChild(o);
  return o;
}
function closeSheet() { document.querySelectorAll('.overlay').forEach((o) => o.remove()); }

function confirmBox(text, okLabel = t('confirm'), danger = false) {
  return new Promise((res) => {
    const o = sheet(`<header><h2>${t('confirm')}</h2></header><div class="body"><p>${esc(text)}</p></div>
      <footer><button class="btn ghost" data-r="0">${t('cancel')}</button><button class="btn ${danger ? 'danger' : 'primary'}" data-r="1">${esc(okLabel)}</button></footer>`);
    o.querySelectorAll('[data-r]').forEach((b) => b.addEventListener('click', () => { closeSheet(); res(b.dataset.r === '1'); }));
  });
}

function md(text) {
  const lines = esc(text).split('\n');
  let html = '', list = false;
  const inline = (s) => s.replace(/\*\*(.+?)\*\*/g, '<strong>$1</strong>').replace(/`([^`]+)`/g, '<code>$1</code>').replace(/\[([^\]]+)\]\([^)]+\)/g, '$1');
  for (const l of lines) {
    const h = l.match(/^(#{1,4})\s+(.*)/);
    const li = l.match(/^\s*[-*]\s+(.*)/);
    if (li) { if (!list) { html += '<ul>'; list = true; } html += `<li>${inline(li[1])}</li>`; continue; }
    if (list) { html += '</ul>'; list = false; }
    if (h) html += `<h${h[1].length}>${inline(h[2])}</h${h[1].length}>`;
    else if (l.trim()) html += `<p>${inline(l)}</p>`;
  }
  return html + (list ? '</ul>' : '');
}

function notesSheet(which) {
  const r = which === 'launcher' ? S.launcherRel : S.releases.find((x) => x.tag === which);
  if (!r) return;
  sheet(`<header><h2>${esc(r.name || r.tag)}</h2><button class="tb-btn" data-close>${icon('x')}</button></header>
    <div class="body notes">${md(r.notes || '')}</div>
    <footer>${r.url ? `<button class="btn ghost" data-url="${esc(r.url)}">${icon('ext')}GitHub</button>` : ''}<button class="btn primary" data-close>${t('ok')}</button></footer>`, 'wide');
  bindSheet();
}

function bindSheet() {
  const o = $('.overlay');
  if (!o) return;
  o.querySelectorAll('[data-close]').forEach((b) => b.addEventListener('click', closeSheet));
  o.querySelectorAll('[data-url]').forEach((b) => b.addEventListener('click', () => openUrl(b.dataset.url)));
}

// ---------- settings ----------
// The game's settings live on the main page (Gráficos · Controles · Juego); the gear keeps only the launcher's own (language,
// Wine/Proton, build processes).
let gameTab = 'graphics';
let settingsTab = 'general';

const NAMED = { '1280x720': '720p', '1920x1080': '1080p', '2560x1440': '1440p', '3440x1440': 'UW 1440p', '3840x2160': '4K', '4096x2160': 'DCI 4K', '5120x2880': '5K', '7680x4320': '8K' };
const resLabel = (m) => { const [w, h] = m.split('x'); return `${w} × ${h}${NAMED[m] ? ' · ' + NAMED[m] : ''}`; };
const seg = (key, opts, val, dis = {}) => `<div class="seg">${opts.map(([v, l]) => `<button data-opt="${key}" data-v="${v}" class="${String(val) === v ? 'on' : ''}" ${dis[v] ? 'disabled' : ''}>${l}</button>`).join('')}</div>`;
const sw = (key, on, attr = 'data-opt') => `<button class="switch ${on ? 'on' : ''}" ${attr}="${key}" data-v="${on ? '0' : '1'}"></button>`;

function gameSettings(inst) {
  const o = S.opts[inst.path];
  if (!o) return '';
  const hw = S.hw || { displays: [], pads: [] };
  const r = S.st.runners;
  const screenCur = hw.displays.map((d) => d.current).filter(Boolean);
  const modes = [...new Set([...hw.displays.flatMap((d) => d.modes), '3840x2160', '2560x1440', '1920x1080', o.SALIDA])]
    .filter((m) => /^\d+x\d+$/.test(m)).sort((a, b) => { const [aw, ah] = a.split('x').map(Number), [bw, bh] = b.split('x').map(Number); return bw * bh - aw * ah || bw - aw; });
  const resOptions = (val, extra = []) => [...extra, ...modes.map((m) => [m, resLabel(m) + (screenCur.includes(m) ? ' — ' + t('yourScreen') : '')])]
    .map(([v, l]) => `<option value="${v}" ${val === v ? 'selected' : ''}>${esc(l)}</option>`).join('');
  let body = '';
  if (gameTab === 'graphics') {
    body = `<div class="group">
      <div class="field"><div class="fl"><b>${t('upscaler')}</b><span>${t('upscalerHint')}</span></div>${seg('REESCALADO', [['taau', t('taau')], ['fsr3', t('fsr3')], ['dlss', t('dlss')]], o.REESCALADO)}</div>
      ${o.REESCALADO === 'fsr3' ? `<div class="field"><div class="fl"><b>${t('sharpness')}</b></div><input type="range" min="0" max="1" step="0.02" value="${esc(o.NITIDEZ)}" data-range="NITIDEZ"><span style="width:44px;text-align:right;color:var(--text-2)">${Math.round(o.NITIDEZ * 100)}%</span></div>` : ''}
      <div class="field"><div class="fl"><b>${t('output')}</b><span>${t('outputHint')}</span></div><div class="mini-select"><select data-optsel="SALIDA">${resOptions(o.SALIDA)}</select></div></div>
      <div class="field"><div class="fl"><b>${t('scale')}</b><span>${t('scaleHint')}</span></div>${seg('ESCALA', [['1', '×1'], ['1.5', '×1.5'], ['2', '×2']], o.ESCALA)}</div>
      <div class="field"><div class="fl"><b>${t('displayMode')}</b></div>${seg('MODO', [['windowed', t('windowed')], ['borderless', t('borderless')]], o.MODO)}</div>
      <div class="field"><div class="fl"><b>${t('windowSize')}</b></div><div class="mini-select"><select data-optsel="VENTANA">${resOptions(o.VENTANA, [['auto', t('screenSize')]])}</select></div></div>
      <div class="field"><div class="fl"><b>${t('lock720')}</b><span>${t('lock720Hint')}</span></div>${sw('BLOQUEAR', o.BLOQUEAR === '1')}</div>
      <div class="field"><div class="fl"><b>${t('fpsCounter')}</b><span>${t('fpsHint')}</span></div>${sw('FPS', o.FPS === '1')}</div>
      ${o.FPS === '1' ? `<div class="field"><div class="fl"><b>${t('fpsCorner')}</b></div>${seg('ESQUINA', [['top_left', t('topLeft')], ['top_right', t('topRight')], ['bottom_left', t('bottomLeft')], ['bottom_right', t('bottomRight')]], o.ESQUINA || 'top_left')}</div>` : ''}
    </div>`;
  } else if (gameTab === 'controls') {
    const pads = hw.pads;
    const known = o.MANDO === 'auto' || pads.some((p) => p.id === o.MANDO);
    body = `<div class="group">
      <div class="field"><div class="fl"><b>${t('controller')}</b><span>${t('controllerHint')}</span></div><div class="mini-select"><select data-optsel="MANDO">
        <option value="auto" ${o.MANDO === 'auto' ? 'selected' : ''}>${t('allControllers')}</option>
        ${pads.map((p) => `<option value="${esc(p.id)}" ${o.MANDO === p.id ? 'selected' : ''}>${esc(p.name)} (${esc(p.id)})</option>`).join('')}
        ${known ? '' : `<option value="${esc(o.MANDO)}" selected>${esc(o.MANDO)} — ${t('notConnected')}</option>`}
      </select></div></div>
      <div class="field"><div class="fl"><b>${t('detected', { n: pads.length })}</b><span>${pads.length ? esc(pads.map((p) => p.name).join(' · ')) : t('noPads')}</span></div><button class="btn sm" data-sact="rescan">${icon('refresh')}${t('rescan')}</button></div>
    </div>`;
  } else {
    body = `<div class="group">
      <div class="field"><div class="fl"><b>${t('gameLanguage')}</b></div><div class="mini-select"><select data-optsel="IDIOMA">
        ${GAME_LANGS.map((g) => `<option value="${g[0]}" ${o.IDIOMA === g[0] ? 'selected' : ''}>${gameLangName(g[0])}</option>`).join('')}</select></div></div>
    </div>`;
  }
  return `<section class="game-settings">
    <div class="inst-head"><div class="label">${t('gameSettings')}</div><span class="hint">${S.st.running ? t('restartNowShort') : t('restartHint')}</span></div>
    <div class="tabs">${['graphics', 'controls', 'game'].map((k) => `<button class="${k === gameTab ? 'on' : ''}" data-gtab="${k}">${t(k)}</button>`).join('')}</div>
    ${body}
  </section>`;
}

async function setOpt(k, v) {
  const inst = current();
  const o = S.opts[inst.path];
  if (!inst || !o) return;
  if (k === 'REESCALADO' && v === 'dlss' && !inst.dlss) return vendorSheet('dlss');
  if (k === 'REESCALADO' && v === 'fsr3' && !inst.fsr) return vendorSheet('fsr');
  const next = { ...o, [k]: v };
  try { await invoke('write_options', { path: inst.path, options: next }); } catch (e) { return toast(err(e), true); }
  S.opts[inst.path] = next;
  render();
  if (S.st.running) offerRestart(); else toast(t('saved'));
}

async function settingsSheet() {
  const inst = current();
  const c = cfg();
  const r = S.st.runners;
  const tabs = ['general', 'advanced'];
  let body = '';
  if (settingsTab === 'general') {
    body = `<div class="group">
      <div class="field"><div class="fl"><b>${t('uiLanguage')}</b></div>${seg('lang', Object.entries(LANGS), lang())}</div>
      <div class="field"><div class="fl"><b>${t('minimizeOnPlay')}</b></div>${sw('minimize_on_play', c.minimize_on_play, 'data-cfg')}</div>
    </div><p style="margin-top:14px;font-size:13px">${t('gameSettingsWhere')}</p>`;
  } else {
    body = `<div class="group">
      <div class="field"><div class="fl"><b>${t('wine')}</b><span>${esc(r.wine || t('notFound'))}</span></div></div>
      <div class="field"><div class="fl"><b>${t('proton')}</b><span>${esc(r.proton || t('notFound'))}</span></div>
        <div class="mini-select"><select data-cfgsel="proton"><option value="">${t('auto')}</option>
        ${r.protons.map((p) => `<option value="${esc(p)}" ${c.proton === p ? 'selected' : ''}>${esc(p.split('/').slice(-2, -1)[0])}</option>`).join('')}</select></div></div>
      <div class="field"><div class="fl"><b>${t('safeRender')}</b><span>${t('safeRenderHint')}</span></div>${sw('safe_render', c.safe_render, 'data-cfg')}</div>
      <div class="field"><div class="fl"><b>${t('jobs')}</b></div><div class="mini-select"><select data-cfgsel="jobs">
        <option value="0">${t('auto')} (${S.st.jobs})</option>${[2, 4, 6, 8, 12, 16, 24, 32].map((n) => `<option value="${n}" ${c.jobs === n ? 'selected' : ''}>${n}</option>`).join('')}</select></div></div>
      ${inst ? `<div class="field"><div class="fl"><b>${esc(instTitle(inst))}</b><span>${esc(inst.path)}</span></div><button class="btn sm" data-url-path="${esc(inst.path)}">${icon('folder')}${t('openFolder')}</button></div>` : ''}
    </div>`;
  }
  const el = sheet(`<header><h2>${t('launcherSettings')}</h2><button class="tb-btn" data-close>${icon('x')}</button></header>
    <div class="tabs">${tabs.map((k) => `<button class="${k === settingsTab ? 'on' : ''}" data-tab="${k}">${t(k)}</button>`).join('')}</div>
    <div class="body">${body}</div>`, '', 'settings');
  bindSheet();
  el.querySelectorAll('[data-tab]').forEach((b) => b.addEventListener('click', () => { settingsTab = b.dataset.tab; settingsSheet(); }));
  el.querySelectorAll('[data-opt=lang]').forEach((b) => b.addEventListener('click', async () => { await invoke('set_config', { patch: { lang: b.dataset.v } }); await refresh(); settingsSheet(); }));
  el.querySelectorAll('[data-cfg]').forEach((b) => b.addEventListener('click', async () => { await invoke('set_config', { patch: { [b.dataset.cfg]: b.dataset.v === '1' } }); await refresh(); settingsSheet(); }));
  el.querySelectorAll('[data-cfgsel]').forEach((s) => s.addEventListener('change', async () => {
    const v = s.dataset.cfgsel === 'jobs' ? parseInt(s.value) : s.value;
    await invoke('set_config', { patch: { [s.dataset.cfgsel]: v } }); await refresh(); settingsSheet();
  }));
  el.querySelectorAll('[data-url-path]').forEach((b) => b.addEventListener('click', () => invoke('open_path', { path: b.dataset.urlPath })));
}

// the game settings on the page: one listener for all of them
document.addEventListener('click', (ev) => {
  if (ev.target.closest('.overlay')) return;
  const tab = ev.target.closest('[data-gtab]');
  if (tab) { gameTab = tab.dataset.gtab; render(); return; }
  if (ev.target.closest('[data-sact=rescan]')) { invoke('hardware').then((h) => { S.hw = h; render(); }); return; }
  const b = ev.target.closest('#view [data-opt]');
  if (b) setOpt(b.dataset.opt, b.dataset.v);
});
document.addEventListener('change', (ev) => {
  if (ev.target.closest('.overlay')) return;
  const s = ev.target.closest('#view [data-optsel], #view [data-range]');
  if (s) setOpt(s.dataset.optsel || s.dataset.range, String(s.value));
});

let restartAsked = false;
async function offerRestart() {
  if (restartAsked) return;
  restartAsked = true;
  const yes = await confirmBox(t('restartNow'), t('restart'));
  restartAsked = false;
  if (!yes) return toast(t('restartHint'));
  await invoke('stop_game');
  for (let i = 0; i < 40 && (await invoke('game_running')); i++) await new Promise((r) => setTimeout(r, 250));
  play();
}

async function vendorSheet(id) {
  const inst = current();
  if (!inst) return;
  const has = id === 'dlss' ? inst.dlss : inst.fsr;
  const title = id === 'dlss' ? t('licenseTitleDlss') : t('licenseTitleFsr');
  const intro = id === 'dlss' ? t('licenseIntroDlss', { v: 'v2.14.16' }) : t('licenseIntroFsr', { v: 'v2.4.0' });
  const vendorName = id === 'dlss' ? 'NVIDIA' : 'AMD';
  let accepted = false;
  const el = sheet(`<header><h2>${title}</h2><button class="tb-btn" data-close>${icon('x')}</button></header>
    <div class="body"><p>${esc(intro)}</p>
      ${id === 'dlss' && !S.st.runners.proton ? `<div class="group proton-box"><div class="field"><div class="fl"><b>${t('protonMissing')}</b><span>${t('protonMissingBody')}</span></div>
        <button class="btn sm accent-outline" id="getproton">${icon('download')}${t('getProton')}</button></div>
        <div class="field" id="pprog" hidden><div class="fl"><div class="bar"><i></i></div><span id="pmsg"></span></div></div></div>` : ''}
      ${has ? `<p style="color:var(--ok);font-weight:600">✓ ${t('installed')}</p>` : ''}
      <div id="lic"><div class="bar indet"><i></i></div></div>
      ${has ? '' : `<div class="check" id="acc"><div class="box">${icon('check')}</div>${esc(t('licenseAccept', { vendor: vendorName }))}</div>`}
      <div id="vprog" style="margin-top:14px" hidden><div class="bar"><i></i></div><p style="margin-top:8px;font-size:13px" id="vmsg"></p></div>
    </div>
    <footer><span class="grow"></span><button class="btn ghost" data-close>${has ? t('ok') : t('cancel')}</button>
      ${has ? '' : `<button class="btn primary" id="vdl" disabled>${icon('download')}${t('download')}</button>`}</footer>`, 'wide');
  bindSheet();
  $('#getproton', el)?.addEventListener('click', async () => {
    const b = $('#getproton', el); b.disabled = true; $('#pprog', el).hidden = false;
    S.vendor = { id: 'proton', el };
    try { await invoke('install_proton'); S.vendor = null; await refresh(); toast(t('protonReady')); vendorSheet(id); }
    catch (e) { S.vendor = null; b.disabled = false; toast(err(e), true); }
  });
  try {
    const lics = await invoke('vendor_license', { id });
    if (!$('#lic', el)) return;
    $('#lic', el).innerHTML = lics.map((l) => `<div class="license"><h4>${esc(l.title)}</h4>${esc(l.text)}</div>`).join('');
  } catch (e) { $('#lic', el).innerHTML = `<p style="color:var(--danger)">${esc(err(e))}</p>`; return; }
  if (has) return;
  const acc = $('#acc', el), dl = $('#vdl', el);
  acc.addEventListener('click', () => { accepted = !accepted; acc.classList.toggle('on', accepted); dl.disabled = !accepted; });
  dl.addEventListener('click', async () => {
    dl.disabled = true; acc.style.pointerEvents = 'none';
    $('#vprog', el).hidden = false;
    S.vendor = { id, el };
    try {
      await invoke('install_vendor', { path: inst.path, id, accepted });
      S.vendor = null;
      toast(t('installed'));
      const o = await invoke('read_options', { path: inst.path });
      await invoke('write_options', { path: inst.path, options: { ...o, REESCALADO: id === 'dlss' ? 'dlss' : 'fsr3' } });
      S.opts[inst.path] = { ...o, REESCALADO: id === 'dlss' ? 'dlss' : 'fsr3' };
      closeSheet();
      await refresh();
    } catch (e) { S.vendor = null; toast(err(e), true); dl.disabled = false; acc.style.pointerEvents = ''; }
  });
}

// money and the three stackable core crystals of the save (null: still reading)
function editFields(v) {
  const rows = [['money', 'money', 99999999], ['common', 'crystalCommon', 99], ['rare', 'crystalRare', 99], ['legendary', 'crystalLegendary', 99], ['overdrive', 'overdrive', 99]];
  return rows.map(([k, label, max]) => `<div class="field"><div class="fl"><b>${t(label)}</b><span>${t('upTo', { n: max.toLocaleString() })}</span></div>
    <input class="num" type="number" min="0" max="${max}" step="1" data-k="${k}" value="${v ? v[k] : ''}" ${v ? '' : 'disabled'}></div>`).join('');
}

async function savesSheet() {
  const inst = current();
  if (!inst) return;
  const s = await invoke('saves', { path: inst.path });
  S.saves[inst.path] = s;
  const emus = S.st.lichii || [];
  const el = sheet(`<header><h2>${t('savesTitle')}</h2><button class="tb-btn" data-close>${icon('x')}</button></header>
    <div class="body"><p>${t('savesIntro')}</p>
      <div class="group">${s.files.length ? s.files.map((f) => `<div class="field"><div class="fl"><b>${esc(f.name)}</b><span>${fmtSize(f.size)} · ${fmtDate(f.modified)}</span></div></div>`).join('')
        : `<div class="field"><div class="fl"><span>${t('savesNone')}</span></div></div>`}</div>
      <p style="margin-top:10px;font-size:12.5px;color:var(--text-3)">${esc(s.folder)}${s.backups ? ' · ' + t('backups', { n: s.backups }) : ''}</p>
      <div class="group">
        ${emus.map((e, i) => `<div class="field"><div class="fl"><b>${esc(t('importFrom', { emu: e.emulator }))}</b><span>${e.files} .sav · ${fmtDate(e.modified)}</span></div><button class="btn sm accent-outline" data-emu="${i}">${icon('download')}${t('importFrom', { emu: e.emulator })}</button></div>`).join('')}
        <div class="field"><div class="fl"><b>${t('importFolder')}</b></div><button class="btn sm" data-s="folder">${icon('folder')}${t('importFolder')}</button></div>
        <div class="field"><div class="fl"><b>${t('backup')}</b></div><button class="btn sm" data-s="backup">${icon('save')}${t('backup')}</button></div>
        <div class="field"><div class="fl"><b>${t('openFolder')}</b></div><button class="btn sm" data-s="open">${icon('ext')}${t('openFolder')}</button></div>
      </div>
      <h3 style="margin:22px 0 6px">${t('editTitle')}</h3><p>${t('editIntro')}</p>
      <div class="group" data-edit>${editFields(null)}</div>
      <div class="acts" style="margin-top:12px;justify-content:flex-end;gap:10px">
        ${s.backups ? `<button class="btn sm" data-s="restore">${icon('ext')}${t('restoreBackup')}</button>` : ''}
        <button class="btn sm primary" data-s="write" disabled>${icon('save')}${t('saveChanges')}</button>
      </div>
    </div>`, 'wide', 'saves');
  bindSheet();
  const box = el.querySelector('[data-edit]');
  const write = el.querySelector('[data-s=write]');
  invoke('save_values', { path: inst.path }).then((v) => { box.innerHTML = editFields(v); write.disabled = false; })
    .catch((e) => { box.innerHTML = `<div class="field"><div class="fl"><span>${esc(err(e))}</span></div></div>`; });
  write.addEventListener('click', async () => {
    if (S.st.running || (await invoke('game_running'))) return toast(t('closeGameEdit'), true);
    const v = {};
    box.querySelectorAll('input.num').forEach((i) => { v[i.dataset.k] = Math.max(0, Math.min(+i.max, Math.floor(+i.value || 0))); });
    try { const r = await invoke('set_save_values', { path: inst.path, values: v }); toast(t('edited', { b: r.backup ? base(r.backup) : '-' })); }
    catch (e) { toast(err(e), true); }
    await refresh(); savesSheet();
  });
  el.querySelector('[data-s=restore]')?.addEventListener('click', async () => {
    if (S.st.running || (await invoke('game_running'))) return toast(t('closeGameEdit'), true);
    if (!(await confirmBox(t('confirmRestore'), t('confirm')))) return savesSheet();
    try { const b = await invoke('restore_backup', { path: inst.path }); toast(t('restored', { b: base(b) })); }
    catch (e) { toast(err(e), true); }
    await refresh(); savesSheet();
  });
  const doImport = async (from, name) => {
    if (S.st.running) return toast(t('closeGameFirst'), true);
    if (!(await confirmBox(t('confirmImport', { emu: name }), t('confirm')))) return savesSheet();
    try { const r = await invoke('import_saves', { path: inst.path, from }); toast(t('imported', { n: r.copied, b: r.backup ? base(r.backup) : '-' })); }
    catch (e) { toast(err(e), true); }
    await refresh(); savesSheet();
  };
  el.querySelectorAll('[data-emu]').forEach((b) => b.addEventListener('click', () => { const e = emus[+b.dataset.emu]; doImport(e.path, e.emulator); }));
  el.querySelector('[data-s=folder]').addEventListener('click', async () => { const p = await invoke('pick', { kind: 'folder', title: t('importFolder') }); if (p) doImport(p, base(p)); });
  el.querySelector('[data-s=backup]').addEventListener('click', async () => { try { const b = await invoke('backup_saves', { path: inst.path }); if (b) toast(t('backupDone', { b: base(b) })); savesSheet(); } catch (e) { toast(err(e), true); } });
  el.querySelector('[data-s=open]').addEventListener('click', () => invoke('open_path', { path: s.folder }).catch((e) => toast(err(e), true)));
}

function menu(x, y, items) {
  document.querySelectorAll('.menu').forEach((m) => m.remove());
  const m = document.createElement('div');
  m.className = 'menu';
  m.innerHTML = items.map((it, i) => `<button data-i="${i}" class="${it.danger ? 'danger' : ''}">${icon(it.icon)}${esc(it.label)}</button>`).join('');
  document.body.appendChild(m);
  const r = m.getBoundingClientRect();
  m.style.left = Math.min(x, innerWidth - r.width - 12) + 'px';
  m.style.top = Math.min(y, innerHeight - r.height - 12) + 'px';
  m.querySelectorAll('button').forEach((b) => b.addEventListener('click', () => { m.remove(); items[+b.dataset.i].run(); }));
  setTimeout(() => document.addEventListener('mousedown', function off(e) { if (!m.contains(e.target)) { m.remove(); document.removeEventListener('mousedown', off); } }), 0);
}

// ---------- actions ----------
function toast(text, isErr = false) {
  const d = document.createElement('div');
  d.className = 'toast' + (isErr ? ' err' : '');
  d.textContent = text;
  $('#toast').appendChild(d);
  setTimeout(() => d.remove(), 4200);
}
function openUrl(url) { if (T) invoke('open_url', { url }).catch((e) => toast(err(e), true)); else window.open(url); }

async function play() {
  const inst = current();
  try {
    const r = await invoke('play', { path: inst.path });
    toast(t('started', { runner: r.proton ? 'Proton' : 'Wine' }));
    S.st.running = true; render();
  } catch (e) {
    if (String(e) === 'no-dlss' || String(e) === 'no-proton') return vendorSheet('dlss');
    toast(err(e), true);
  }
}

async function startBuild(mode, inst, release) {
  const w = S.wiz;
  const rel = release || (w.zip ? null : S.releases.find((r) => r.tag === w.tag));
  const req = {
    instance: inst || w.folder.trim(),
    source: mode === 'update' ? '' : (w.found || w.source),
    release: rel || null,
    local_zip: mode === 'update' ? null : (w.zip || null),
    channel: cfg().channel || 'estable',
    mode,
    wine: '', jobs: 0,
  };
  if (release) S.wiz.tag = release.tag;
  try {
    S.log = []; S.showLog = false;
    await invoke('start_build', { request: req });
    S.build = { state: 'running', step_index: 0, instance: req.instance };
    S.view = 'build'; render();
  } catch (e) { toast(err(e), true); }
}

const actions = {
  play,
  stop: async () => { await invoke('stop_game'); setTimeout(refresh, 800); },
  continue: () => { const i = current(); startBuild('new', i.path, null); },
  update: async () => {
    const inst = current();
    const up = updateFor(inst);
    if (up) startBuild('update', inst.path, up);
  },
  'make-default': async (d) => { await invoke('set_default', { path: d.path }); refresh(); },
  select: (d) => { S.sel = d.path; render(); },
  remove: async (d) => {
    if (!(await confirmBox(t('removeConfirm', { name: base(d.path) }), t('remove'), true))) return;
    await invoke('forget_instance', { path: d.path });
    S.sel = null; refresh();
  },
  menu: (d, ev) => {
    const r = ev.target.closest('button').getBoundingClientRect();
    menu(r.left - 180, r.bottom + 6, [
      { icon: 'folder', label: t('openFolder'), run: () => invoke('open_path', { path: d.path }) },
      { icon: 'star', label: t('makeDefault'), run: () => actions['make-default'](d) },
      { icon: 'trash', label: t('remove'), danger: true, run: () => actions.remove(d) },
    ]);
  },
  saves: savesSheet,
  vendor: (d) => vendorSheet(d.id),
  notes: (d) => notesSheet(d.which),
  dismiss: async (d) => { await invoke('set_config', { patch: { dismissed: d.which } }); refresh(); },
  'open-url': (d) => openUrl(d.url),
  'update-launcher': async () => {
    // a window with the download bar; when it is done the launcher closes and the new one opens by itself
    const v = S.launcherRel?.version || '';
    const el = sheet(`<header><h2>${esc(t('updatingLauncher', { v }))}</h2></header>
      <div class="body"><div class="bar" style="margin:6px 0 10px"><i id="lup-bar"></i></div><p id="lup-msg" style="font-size:13px">${t('wineInstalling')}</p></div>`, '', 'launcher-update');
    S.launcherDl = el;
    try { await invoke('update_launcher', { release: S.launcherRel }); }
    catch (e) { S.launcherDl = null; closeSheet(); toast(err(e), true); }
  },
  'check-updates': () => loadReleases(true),
  'add-folder': async () => {
    const p = await invoke('pick', { kind: 'folder', title: t('addFolder') });
    if (!p) return;
    try { const i = await invoke('add_instance', { path: p }); S.sel = i.path; S.view = 'main'; await refresh(); }
    catch (e) { toast(err(e), true); }
  },
  'build-other': () => { S.wiz = { source: cfg().last_source || '', check: null, tag: '', zip: '', folder: '', mode: 'new' }; if (S.wiz.source) checkSource(); S.view = 'setup'; render(); },
  back: () => { S.view = 'main'; render(); },
  'pick-source': async (d) => {
    const p = await invoke('pick', { kind: d.kind === 'nup' ? 'nup' : 'folder', title: d.kind === 'nup' ? t('pickNup') : t('pickFolder') });
    if (p) { S.wiz.source = p; S.wiz.found = ''; checkSource(); }
  },
  'pick-zip': async () => { const p = await invoke('pick', { kind: 'zip', title: t('useZip') }); if (p) { S.wiz.zip = p; S.wiz.tag = ''; render(); } },
  'pick-folder': async () => {
    const p = await invoke('pick', { kind: 'folder', title: t('stepFolder') });
    if (p) { S.wiz.folder = p.endsWith('/xc2-recomp') ? p : p; render(); }
  },
  'start-build': () => startBuild('new', null, null),
  'wine-install': async () => {
    S.wineDl = { done: 0, total: 0, msg: t('wineInstalling') }; render();
    try { await invoke('install_wine'); S.wineDl = null; await refresh(); toast(t('wineReady')); }
    catch (e) { S.wineDl = null; render(); toast(err(e), true); }
  },

  'cancel-build': () => invoke('cancel_build'),
  'retry-build': () => { const b = S.build; startBuild('new', b.instance, S.releases.find((r) => r.tag === S.wiz.tag) || null); },
  'build-finished': async () => { S.view = instances().length || S.build?.state === 'done' ? 'main' : 'setup'; if (S.build?.instance) S.sel = S.build.instance; S.build = null; await refresh(); },
  'toggle-log': async () => { S.showLog = !S.showLog; if (S.showLog && !S.log.length) S.log = await invoke('build_log'); render(); },
};

async function checkSource() {
  S.wiz.check = null; render();
  S.wiz.check = await invoke('check_source', { path: S.wiz.source });
  // what was found inside the chosen folder is what Set up uses
  if (S.wiz.check?.ok && S.wiz.check.path) S.wiz.found = S.wiz.check.path;
  render();
}

document.addEventListener('click', (ev) => {
  const el = ev.target.closest('[data-act]');
  if (!el || el.closest('.overlay')) return;
  ev.stopPropagation();
  const a = actions[el.dataset.act];
  if (a) a(el.dataset, ev);
}, true);

document.addEventListener('change', async (ev) => {
  const el = ev.target.closest('[data-change]');
  if (!el || el.closest('.overlay')) return;
  const k = el.dataset.change;
  if (k === 'channel') { await invoke('set_config', { patch: { channel: el.value } }); await refresh(); }
  else if (k === 'select-instance') { S.sel = el.value; render(); }
  else if (k === 'wiz-tag') { if (!el.value.startsWith('.zip') && S.releases.some((r) => r.tag === el.value)) { S.wiz.tag = el.value; S.wiz.zip = ''; S.wiz.folder = ''; render(); } }
});
document.addEventListener('input', (ev) => { const el = ev.target.closest('[data-input=wiz-folder]'); if (el) S.wiz.folder = el.value; });
document.addEventListener('keydown', (ev) => { if (ev.key === 'Escape') { closeSheet(); document.querySelectorAll('.menu').forEach((m) => m.remove()); } });

// ---------- title bar ----------
$('#btn-settings').innerHTML = icon('gear');
$('#btn-min').innerHTML = icon('min');
$('#btn-close').innerHTML = icon('x');
$('#btn-settings').addEventListener('click', () => { settingsTab = 'general'; settingsSheet(); });
$('#btn-min').addEventListener('click', () => win?.minimize());
// maximize / restore: the icon and the square corners follow the window's state
async function syncMax() {
  const m = win ? await win.isMaximized().catch(() => false) : false;
  document.documentElement.classList.toggle('maximized', m);
  $('#btn-max').innerHTML = icon(m ? 'restore' : 'max');
  $('#btn-max').title = m ? t('restoreWin') : t('maximizeWin');
}
$('#btn-max').addEventListener('click', async () => { await win?.toggleMaximize(); setTimeout(syncMax, 120); });
if (win) win.onResized(() => syncMax());
syncMax();
$('#btn-close').addEventListener('click', () => win?.close());
document.querySelector('.titlebar').addEventListener('dblclick', (e) => { if (e.target.closest('[data-tauri-drag-region]')) win?.toggleMaximize(); });

// ---------- events ----------
listen('build-progress', async (e) => {
  S.build = e.payload;
  if (S.view === 'build') render();
  if (e.payload.state === 'done') {
    // finished: back to the main page by itself, with the install as it is now
    const done = e.payload.instance;
    setTimeout(async () => {
      if (S.view !== 'build' || S.build?.state !== 'done') return;
      S.view = 'main'; if (done) S.sel = done; S.build = null;
      await refresh();
      const i = instances().find((x) => x.path === done);
      toast(t('gameUpdated', { v: i?.tag || '' }));
    }, 1800);
  } else if (e.payload.state !== 'running') refresh();
});
listen('build-log', (e) => {
  S.log.push(...e.payload);
  if (S.log.length > 4000) S.log.splice(0, S.log.length - 3000);
  const l = $('.log');
  if (l) { l.textContent = S.log.slice(-600).join('\n'); l.scrollTop = l.scrollHeight; }
});
// the game ended with an error within its first minute: show why instead of "starting…"
listen('game-exit', (e) => {
  const { code, log } = e.payload;
  if (S.st) S.st.running = false;
  render();
  sheet(`<header><h2>${t('gameExitTitle')}</h2><button class="tb-btn" data-close>${icon('x')}</button></header>
    <div class="body"><p>${esc(t('gameExitBody', { code }))}</p><div class="log" style="height:260px">${esc(log || '')}</div></div>
    <footer><button class="btn primary" data-close>${t('ok')}</button></footer>`, 'wide', 'game-exit');
  bindSheet();
});
listen('vendor-progress', (e) => {
  const v = S.vendor; const p = e.payload;
  if (p.phase === 'launcher' && S.launcherDl && document.body.contains(S.launcherDl)) {
    const f = p.total ? p.done / p.total : 0;
    S.launcherDl.querySelector('#lup-bar').style.width = (f * 100).toFixed(1) + '%';
    S.launcherDl.querySelector('#lup-msg').textContent = f >= 1 ? t('restartingLauncher') : t('downloading', { p: `${fmtSize(p.done)} / ${fmtSize(p.total)}` });
    return;
  }
  if (p.id === 'wine') {
    S.wineDl = { done: p.done, total: p.total, msg: p.phase === 'extract' ? t('extracting') : t('downloading', { p: `${fmtSize(p.done)} / ${fmtSize(p.total)}` }) };
    lastHtml = ''; render();
    return;
  }
  if (!v || !document.body.contains(v.el)) return;
  const isP = p.id === 'proton';
  const bar = v.el.querySelector(isP ? '#pprog .bar i' : '#vprog .bar i'); const msg = v.el.querySelector(isP ? '#pmsg' : '#vmsg');
  if (!bar || !msg) return;
  if (p.phase === 'verify' || p.phase === 'extract') { msg.textContent = p.phase === 'verify' ? t('verifying') : t('extracting'); bar.style.width = '100%'; return; }
  const f = p.total ? p.done / p.total : 0;
  bar.style.width = (f * 100).toFixed(1) + '%';
  msg.textContent = p.phase === 'extract' ? t('extracting') : t('downloading', { p: `${fmtSize(p.done)} / ${fmtSize(p.total)}` });
});
setInterval(async () => {
  if (!S.st || S.view !== 'main') return;
  const r = await invoke('game_running');
  if (r !== S.st.running) { S.st.running = r; render(); }
}, 2500);

refresh().then(() => loadReleases(false));

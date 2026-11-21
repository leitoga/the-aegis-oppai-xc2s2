// SPDX-License-Identifier: GPL-3.0-or-later
// A stand-in backend for working on the page in a plain browser (no Tauri): `?demo=main|first|build`. Never used in the app.
const demo = new URLSearchParams(location.search).get('demo') || 'main';
const now = Math.floor(Date.now() / 1000);
const I = '/home/player/Games/xc2-ns2-recomp';
let config = { lang: new URLSearchParams(location.search).get('lang') || '', instances: demo === 'first' ? [] : [I], default: I, channel: 'estable', minimize_on_play: false, jobs: 0, proton: '', wine: '', dismissed: '' };
const opts = { VENTANA: 'auto', MODO: 'borderless', BLOQUEAR: '0', FPS: '1', SALIDA: '3840x2160', ESCALA: '1', REESCALADO: 'dlss', NITIDEZ: '1', IDIOMA: 'LatinAmericanSpanish', MANDO: 'auto', ESQUINA: 'top_left' };
const rel = (v, d, pre = false) => ({ tag: 'v' + v, version: v, name: `XC2 NS2 Recomp v${v}`, prerelease: pre, date: d, url: 'https://github.com/weile10000/xc2-ns2-recomp/releases/tag/v' + v,
  notes: `# XC2 NS2 Recomp v${v}\n\n**Demo notes.**\n\n- The frame on screen is the frame the game finished\n- Cutscenes have their sound back\n- \`NXRT_NVN_PRESENT_AHEAD\` defaults to 0`,
  asset_name: `XC2-package-dist-${v}.zip`, asset_url: 'x', asset_size: 674634422, sha256: '' });
const listeners = {};
export function mockListen(ev, cb) { (listeners[ev] ||= []).push(cb); }
const emit = (ev, payload) => (listeners[ev] || []).forEach((cb) => cb({ payload }));

export async function mockInvoke(cmd, a = {}) {
  await new Promise((r) => setTimeout(r, 40));
  switch (cmd) {
    case 'get_state':
      return {
        config, version: '0.1.0', running: false, system_lang: 'es_AR.UTF-8', jobs: 11,
        building: { state: demo === 'build' ? 'running' : 'idle', step_index: 4, step: 'link', fraction: 0.62, message: '333/537', instance: '/home/player/Games/xc2-recomp-0.1.5' },
        runners: { wine: new URLSearchParams(location.search).get('nowine') ? null : '/usr/bin/wine', proton: new URLSearchParams(location.search).get('nowine') ? null : '/usr/share/steam/compatibilitytools.d/proton-cachyos-slr/proton', protons: ['/usr/share/steam/compatibilitytools.d/proton-cachyos-slr/proton'], gamemode: true, mangohud: true },
        lichii: [{ emulator: 'Lichii', path: '/home/player/.local/share/lichii/nand/user/save/0000000000000000/0000/0100E95004038000', files: 2, modified: now - 86400 * 2 }],
        instances: config.instances.map((p) => ({ path: p, display: p.replace('/home/player', '~'), exists: true, version: '0.1.4', tag: 'v0.1.4', channel: 'estable', built: now - 3600 * 9, ready: true, dlss: true, fsr: true, language: opts.IDIOMA, has_cover: true, upscaler: opts.REESCALADO })),
      };
    case 'releases': return { game: [rel('0.1.5', '2026-11-22T10:00:00Z'), rel('0.1.4', '2026-11-20T20:29:37Z'), rel('0.1.3', '2026-11-19T08:59:12Z')], launcher: new URLSearchParams(location.search).get('lup') ? rel('0.2.0', '') : null, error: null };
    case 'cover': return 'cover-demo.jpg';
    case 'instance_size': return 8.0e9;
    case 'saves': return { folder: I + '/pkg/save/account/0100000000000000635232534e324358', files: [{ name: 'bf2savefile.sav', size: 2101248, modified: now - 7200 }, { name: 'bf2option.sav', size: 4096, modified: now - 7200 }], latest: now - 7200, backups: 1 };
    case 'read_options': return { ...opts };
    case 'write_options': Object.assign(opts, a.options); return null;
    case 'set_config': Object.assign(config, a.patch); return null;
    case 'game_running': return false;
    case 'install_wine': await new Promise((r) => setTimeout(r, 1500)); return '/home/player/.local/share/aegis-oppai/wine-11.22-amd64-wow64/bin/wine';
    case 'hardware': return { displays: [{ name: 'HDMI-A-2', current: '3840x2160', modes: ['4096x2160', '3840x2160', '2560x1440', '1920x1080', '1680x1050', '1600x900', '1440x900', '1280x1024', '1280x800', '1280x720', '1152x864', '1024x768'] }], pads: [{ name: 'Microsoft X-Box 360 pad', id: '045e:028e' }] };
    case 'build_log': return ['instance /home/player/Games/xc2-recomp-0.1.5', '$ "/usr/bin/wine"', '  STEP: lift done 59.6s', '  333 of 537 to compile'];
    case 'suggest_folder': return '/home/player/Games/xc2-recomp-' + (a.version || 'nueva');
    case 'check_source': return { ok: true, kind: 'folder', detail: 'exefs + romfs · ~/Games/xc2-ns2-recomp/dump', path: '/home/player/Games/xc2-ns2-recomp/dump' };
    case 'pick': return a.kind === 'nup' ? '/home/player/Games/dumps/xc2-3.0.1.nup' : '/home/player/Games/xc2-ns2-recomp/dump';
    case 'vendor_license': await new Promise((r) => setTimeout(r, 400)); return [{ title: 'NVIDIA DLSS (NGX SDK) — nvngx_dlss.dll', url: '', text: 'NVIDIA RTX SDKs LICENSE\n\nThis license is a legal agreement between you and NVIDIA Corporation ("NVIDIA")…\n\n(demo text)' }];
    case 'start_build': setTimeout(() => emit('build-progress', { state: 'running', step_index: 2, step: 'lift', fraction: -1, message: '', instance: a.request.instance }), 600); return null;
    default: return null;
  }
}

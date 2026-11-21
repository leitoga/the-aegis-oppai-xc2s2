// SPDX-License-Identifier: GPL-3.0-or-later
// The launcher's texts: Latin American Spanish and English. t('key', {var}) fills {var}.
export const LANGS = { es: 'Español (Latinoamérica)', en: 'English' };

const es = {
  appName: 'The Aegis’s Oppai',
  gameTitle: 'The Aegis’s Oppai',
  gameSub: '<b>XC2S2 Edition</b> · Xenoblade Chronicles 2 · {version}',
  settings: 'Configuración', minimize: 'Minimizar', close: 'Cerrar', maximizeWin: 'Maximizar', restoreWin: 'Restaurar',
  // banner
  newLauncher: 'Salió un nuevo launcher',
  newLauncherBody: 'Salió The Aegis’s Oppai v{new} (tienes la v{cur}).',
  newRelease: 'Salió una nueva versión del port',
  newReleaseBody: 'Salió xc2-ns2-recomp {new} (tienes la {cur}).',
  details: 'Detalles', whatsNew: 'Novedades', updateLauncher: 'Actualizar el launcher', update: 'Actualizar', later: 'Más tarde',
  // left column
  channel: 'Canal', version: 'Versión', stable: 'Estable', preview: 'Preliminar',
  updateAvailable: 'Actualización disponible',
  play: 'Jugar', playing: 'Jugando', stop: 'Cerrar juego', building: 'Compilando…',
  defaultNote: 'Tu versión predeterminada.', notDefaultNote: 'No es tu versión predeterminada.', makeDefault: 'Hacer predeterminada',
  builtOn: 'Compilado: {date}', remove: 'Quitar',
  notReady: 'Esta instancia todavía no está compilada.', continueSetup: 'Continuar la configuración',
  savesTitle: 'Administrar partidas', savesSub: '{n} partidas, la última {date}', savesNone: 'Todavía no hay partidas',
  dlssTitle: 'NVIDIA DLSS', dlssOff: 'Activar DLSS de NVIDIA', dlssOn: 'Instalado · Streamline {v}',
  fsrTitle: 'AMD FSR 3.1', fsrOff: 'Descargar FSR 3.1 de AMD', fsrOn: 'Instalado · FidelityFX SDK {v}',
  // right column
  releaseOut: 'Ya salió {name}', instances: 'Tus instancias',
  addFolder: 'Agregar una carpeta de compilación…', buildOther: 'Compilar otra versión',
  defaultBadge: 'Predeterminada', notBuilt: 'Sin compilar', missing: 'No se encuentra',
  usage: '{n} instancia ocupa {size} en {dir}.', usagePlural: '{n} instancias ocupan {size}.', measuring: 'Midiendo…',
  checkUpdates: 'Buscar actualizaciones', checking: 'Buscando…', upToDate: 'Estás al día.', offline: 'No se pudo consultar GitHub: {e}',
  openFolder: 'Abrir carpeta', setDefault: 'Predeterminada',
  // feature line
  langLatam: 'Español (Latinoamérica)',
  // first run
  welcome: 'Bienvenido', welcomeBody: 'The Aegis’s Oppai XC2S2 Edition compila Xenoblade Chronicles 2 en tu PC a partir de tu propio volcado del juego y lo arranca con Wine o Proton. No incluye archivos del juego.',
  haveInstall: '¿Ya tienes una instalación?', haveInstallBody: 'Agrega la carpeta que contiene pkg/ (por ejemplo ~/Games/xc2-ns2-recomp).',
  stepFiles: 'Tus archivos del juego', stepFilesBody: 'Elige tu paquete .nup o la carpeta con exefs/ y romfs/ (versión 3.0.1).',
  pickNup: 'Elegir .nup…', pickFolder: 'Elegir carpeta…',
  stepVersion: 'Versión del port', useZip: 'Usar un .zip que ya tengo…', fromGithub: 'Desde GitHub',
  stepFolder: 'Carpeta de instalación', change: 'Cambiar…',
  setUp: 'Configurar', setUpHint: 'Tarda entre 10 y 20 minutos: traduce el código del juego y unos 12.000 shaders.',
  cancel: 'Cancelar', back: 'Volver', done: 'Listo', retry: 'Reintentar', showLog: 'Ver registro', hideLog: 'Ocultar registro',
  srcOk: '{kind} · {detail}', srcBad: 'No parece un volcado válido.',
  // build steps
  step_package: 'Descargar y descomprimir el paquete', step_prefix: 'Preparar Wine', step_extract: 'Extraer el juego del .nup', step_lift: 'Traducir el código del juego',
  step_zig: 'Preparar el compilador (zig)', step_link: 'Compilar xc2_boot.exe', step_shaders: 'Traducir los shaders', step_done: 'Terminar',
  buildTitle: 'Compilando {version}', buildDone: '¡Listo para jugar!', buildError: 'La compilación se detuvo', buildCancelled: 'Compilación pausada. Puedes continuarla más tarde.',
  // settings
  general: 'General', graphics: 'Gráficos', game: 'Juego', advanced: 'Avanzado',
  uiLanguage: 'Idioma del launcher', minimizeOnPlay: 'Minimizar el launcher al jugar',
  upscaler: 'Reescalado', upscalerHint: 'Lo que en la consola hace el DLSS. DLSS usa Proton; TAAU y FSR usan Wine.',
  taau: 'TAAU', fsr3: 'FSR 3.1', dlss: 'DLSS',
  sharpness: 'Nitidez (FSR 3.1)',
  output: 'Resolución de salida', outputHint: 'Todas las resoluciones de tu monitor. El reescalador lleva el 3D a esta resolución; una mayor que tu pantalla se achica al final (más nítido). 1080p, 1440p y 4K son las probadas.',
  scale: 'Escala del 3D', scaleHint: '×1 es 720p como la consola. Más alto es más nítido y más pesado.',
  displayMode: 'Pantalla', windowed: 'Ventana', borderless: 'Sin bordes',
  windowSize: 'Tamaño de la ventana', screenSize: 'Tamaño de la pantalla',
  lock720: 'Fijar la resolución interna en 720p', lock720Hint: 'El juego baja su resolución cuando hay mucha carga. Fijarla mantiene la imagen nítida (cuesta 1-2 fps).',
  fpsCounter: 'Contador de FPS', fpsMissing: '', fpsHint: 'Solo «FPS 60» en una esquina, dibujado por el propio port.', fpsCorner: 'Esquina del contador', topLeft: 'Arriba izq.', topRight: 'Arriba der.', bottomLeft: 'Abajo izq.', bottomRight: 'Abajo der.',
  gameLanguage: 'Idioma del juego', cover: 'Portada', coverHint: 'Una imagen de tu propio volcado (por ejemplo el ícono del juego).', pickCover: 'Elegir imagen…',
  wine: 'Wine', proton: 'Proton (para DLSS)', auto: 'Automático', notFound: 'No encontrado', jobs: 'Procesos al compilar',
  restartHint: 'Los cambios se aplican la próxima vez que abras el juego.',
  restartNow: 'El juego está abierto. ¿Reiniciarlo para aplicar los cambios? Se pierde lo que no hayas guardado.',
  restart: 'Reiniciar', saved: 'Guardado',
  needDlss: 'Para DLSS hay que descargar los archivos de NVIDIA. Abre la tarjeta “NVIDIA DLSS”.',
  needFsr: 'Para FSR 3.1 hay que descargar los archivos de AMD.',
  // license
  licenseTitleDlss: 'Activar DLSS de NVIDIA',
  licenseTitleFsr: 'Activar FSR 3.1 de AMD',
  licenseIntroDlss: 'DLSS necesita archivos de NVIDIA (Streamline {v}: sl.interposer.dll, sl.common.dll, sl.dlss.dll y nvngx_dlss.dll). No vienen con el launcher: se descargan del repositorio oficial de NVIDIA (unos 276 MB) y quedan solo en esta instancia. Para usarlos tienes que aceptar la licencia de NVIDIA.',
  licenseIntroFsr: 'FSR 3.1 necesita dos archivos de AMD (FidelityFX SDK {v}). No vienen con el launcher: se descargan del repositorio oficial de AMD (unos 36 MB) y quedan solo en esta instancia.',
  licenseAccept: 'Leí y acepto la licencia de {vendor}',
  download: 'Descargar', downloading: 'Descargando… {p}', extracting: 'Descomprimiendo…', installed: 'Instalado',
  dlssNeedsProton: 'DLSS se ejecuta con Proton. No se encontró ninguno: instala proton-cachyos o GE-Proton.',
  // saves
  savesIntro: 'Tus partidas de esta instancia. Antes de importar se hace una copia de seguridad.',
  importFrom: 'Importar de {emu}', importFolder: 'Importar de una carpeta…', backup: 'Hacer copia de seguridad',
  backups: '{n} copias de seguridad', imported: 'Se importaron {n} archivos. Copia de la versión anterior en {b}.',
  backupDone: 'Copia de seguridad en {b}', closeGameFirst: 'Cierra el juego antes de importar partidas.',
  confirmImport: '¿Importar las partidas de {emu}? Reemplaza las de esta instancia (antes se hace una copia).',
  // misc
  started: 'Iniciando el juego con {runner}…', notInstance: 'Esa carpeta no tiene una instalación (falta pkg/).',
  removeConfirm: '¿Quitar {name} de la lista? La carpeta queda en el disco, no se borra nada.',
  confirm: 'Confirmar', ok: 'Aceptar', error: 'Error: {e}',
  e_running: 'El juego ya está abierto.', 'e_no-wine': 'No se encontró Wine.', 'e_no-proton': 'No se encontró Proton (lo necesita DLSS).',
  'e_no-dlss': 'Faltan los archivos de DLSS.', 'e_not-ready': 'La instancia no está compilada.', e_building: 'Hay una compilación en curso.',
  'e_not-instance': 'Esa carpeta no tiene una instalación (falta pkg/).',
  noReleases: 'No hay versiones en este canal.', localZip: '.zip local: {name}',
  wineMissing: 'Falta Wine', wineMissingBody: 'El juego es un programa de Windows y necesita Wine. El launcher baja su propio Wine 11.22 (unos 98 MB, sin contraseña, no se instala nada en el sistema). La configuración inicial también lo baja sola.',
  installWine: 'Descargar Wine', wineInstalling: 'Descargando…', wineReady: 'Wine listo. Ya puedes compilar y jugar.',
  protonMissing: 'Falta Proton', protonMissingBody: 'DLSS corre con Proton. Se descarga GE-Proton (unos 570 MB) en tu carpeta de usuario; no pide contraseña.',
  getProton: 'Descargar GE-Proton', protonReady: 'GE-Proton listo.', verifying: 'Verificando…',
  never: 'nunca',
  controls: 'Controles', controller: 'Mando', controllerHint: 'El juego usa solo este mando (útil si Steam o un adaptador agregan mandos duplicados).',
  allControllers: 'Todos (automático)', notConnected: 'no conectado', detected: 'Mandos conectados: {n}', noPads: 'No se detecta ningún mando.', rescan: 'Buscar de nuevo',
  inGameMenu: 'El port tiene además su propio menú dentro del juego (pestañas Gráficos, Controles, Audio y Juego): ahí se reasignan los botones, se ajusta el reescalado y la imagen.',
  yourScreen: 'tu pantalla',
  safeRender: 'Dibujo compatible', safeRenderHint: 'Más lento. Actívalo solo si la ventana del launcher se ve en blanco o se corta (algunos drivers). Se aplica al reabrir el launcher.',
  gameSettings: 'Configuración del juego', launcherSettings: 'Configuración del launcher', restartNowShort: 'El juego está abierto: los cambios piden reiniciarlo.',
  gameSettingsWhere: 'La configuración del juego (resolución, reescalado, mando, idioma) está en la pantalla principal, debajo de tus instancias.',
  kind_nup: 'Paquete .nup', kind_folder: 'Carpeta', kind_exefs: 'Carpeta exefs', kind_dump: 'Volcado (base + updates)', kind_none: '?',
};

const en = {
  appName: 'The Aegis’s Oppai',
  gameTitle: 'The Aegis’s Oppai',
  gameSub: '<b>XC2S2 Edition</b> · Xenoblade Chronicles 2 · {version}',
  settings: 'Settings', minimize: 'Minimize', close: 'Close', maximizeWin: 'Maximize', restoreWin: 'Restore',
  newLauncher: 'A new launcher is out',
  newLauncherBody: 'The Aegis’s Oppai v{new} is out (you have v{cur}).',
  newRelease: 'A new version of the port is out',
  newReleaseBody: 'xc2-ns2-recomp {new} is out (you have {cur}).',
  details: 'Details', whatsNew: "What's new", updateLauncher: 'Update the launcher', update: 'Update', later: 'Later',
  channel: 'Channel', version: 'Version', stable: 'Stable', preview: 'Preview',
  updateAvailable: 'Update available',
  play: 'Play', playing: 'Playing', stop: 'Close game', building: 'Building…',
  defaultNote: 'Your default version.', notDefaultNote: 'Not your default version.', makeDefault: 'Make default',
  builtOn: 'Built: {date}', remove: 'Remove',
  notReady: 'This instance is not built yet.', continueSetup: 'Continue setup',
  savesTitle: 'Manage saves', savesSub: '{n} saves, the latest {date}', savesNone: 'No saves yet',
  dlssTitle: 'NVIDIA DLSS', dlssOff: 'Turn on NVIDIA DLSS', dlssOn: 'Installed · Streamline {v}',
  fsrTitle: 'AMD FSR 3.1', fsrOff: "Download AMD's FSR 3.1", fsrOn: 'Installed · FidelityFX SDK {v}',
  releaseOut: '{name} is out', instances: 'Your instances',
  addFolder: 'Add a build folder…', buildOther: 'Build another version',
  defaultBadge: 'Default', notBuilt: 'Not built', missing: 'Not found',
  usage: '{n} instance uses {size} in {dir}.', usagePlural: '{n} instances use {size}.', measuring: 'Measuring…',
  checkUpdates: 'Check for updates', checking: 'Checking…', upToDate: "You're up to date.", offline: 'Could not reach GitHub: {e}',
  openFolder: 'Open folder', setDefault: 'Default',
  langLatam: 'Spanish (Latin America)',
  welcome: 'Welcome', welcomeBody: 'The Aegis’s Oppai XC2S2 Edition builds Xenoblade Chronicles 2 on your PC from your own dump of the game and starts it with Wine or Proton. It contains no game files.',
  haveInstall: 'Already have an install?', haveInstallBody: 'Add the folder that holds pkg/ (for example ~/Games/xc2-ns2-recomp).',
  stepFiles: 'Your game files', stepFilesBody: 'Pick your .nup package or the folder with exefs/ and romfs/ (version 3.0.1).',
  pickNup: 'Choose .nup…', pickFolder: 'Choose folder…',
  stepVersion: 'Port version', useZip: 'Use a .zip I already have…', fromGithub: 'From GitHub',
  stepFolder: 'Install folder', change: 'Change…',
  setUp: 'Set up', setUpHint: 'Takes 10 to 20 minutes: it translates the game code and about 12,000 shaders.',
  cancel: 'Cancel', back: 'Back', done: 'Done', retry: 'Retry', showLog: 'Show log', hideLog: 'Hide log',
  srcOk: '{kind} · {detail}', srcBad: 'This does not look like a valid dump.',
  step_package: 'Download and unpack the package', step_prefix: 'Prepare Wine', step_extract: 'Unpack the game from the .nup', step_lift: 'Translate the game code',
  step_zig: 'Get the compiler (zig)', step_link: 'Build xc2_boot.exe', step_shaders: 'Translate the shaders', step_done: 'Finish',
  buildTitle: 'Building {version}', buildDone: 'Ready to play!', buildError: 'The build stopped', buildCancelled: 'Build paused. You can continue it later.',
  general: 'General', graphics: 'Graphics', game: 'Game', advanced: 'Advanced',
  uiLanguage: 'Launcher language', minimizeOnPlay: 'Minimize the launcher when playing',
  upscaler: 'Upscaler', upscalerHint: "What DLSS does on the console. DLSS runs through Proton; TAAU and FSR through Wine.",
  taau: 'TAAU', fsr3: 'FSR 3.1', dlss: 'DLSS',
  sharpness: 'Sharpness (FSR 3.1)',
  output: 'Output resolution', outputHint: 'Every resolution your monitor offers. The upscaler takes the 3D to this size; one larger than your screen is scaled down at the end (sharper). 1080p, 1440p and 4K are the tested ones.',
  scale: '3D scale', scaleHint: '×1 is 720p like the console. Higher is sharper and heavier.',
  displayMode: 'Display', windowed: 'Window', borderless: 'Borderless',
  windowSize: 'Window size', screenSize: 'Screen size',
  lock720: 'Lock the internal resolution at 720p', lock720Hint: 'The game lowers its resolution under load. Locking it keeps the picture sharp (costs 1-2 fps).',
  fpsCounter: 'FPS counter', fpsMissing: '', fpsHint: 'Just “FPS 60” in a corner, drawn by the port itself.', fpsCorner: 'Counter corner', topLeft: 'Top left', topRight: 'Top right', bottomLeft: 'Bottom left', bottomRight: 'Bottom right',
  gameLanguage: 'Game language', cover: 'Cover', coverHint: "An image from your own dump (for example the game's icon).", pickCover: 'Choose image…',
  wine: 'Wine', proton: 'Proton (for DLSS)', auto: 'Automatic', notFound: 'Not found', jobs: 'Build processes',
  restartHint: 'Changes apply the next time you open the game.',
  restartNow: 'The game is open. Restart it to apply the changes? Anything unsaved is lost.',
  restart: 'Restart', saved: 'Saved',
  needDlss: "DLSS needs NVIDIA's files. Open the “NVIDIA DLSS” card.",
  needFsr: "FSR 3.1 needs AMD's files.",
  licenseTitleDlss: 'Turn on NVIDIA DLSS',
  licenseTitleFsr: 'Turn on AMD FSR 3.1',
  licenseIntroDlss: "DLSS needs NVIDIA's files (Streamline {v}: sl.interposer.dll, sl.common.dll, sl.dlss.dll and nvngx_dlss.dll). They do not come with the launcher: they are downloaded from NVIDIA's official repository (about 276 MB) and stay in this instance only. Using them means accepting NVIDIA's license.",
  licenseIntroFsr: "FSR 3.1 needs two AMD files (FidelityFX SDK {v}). They do not come with the launcher: they are downloaded from AMD's official repository (about 36 MB) and stay in this instance only.",
  licenseAccept: "I have read and accept {vendor}'s license",
  download: 'Download', downloading: 'Downloading… {p}', extracting: 'Unpacking…', installed: 'Installed',
  dlssNeedsProton: 'DLSS runs through Proton. None was found: install proton-cachyos or GE-Proton.',
  savesIntro: 'The saves of this instance. A backup is made before anything is imported.',
  importFrom: 'Import from {emu}', importFolder: 'Import from a folder…', backup: 'Make a backup',
  backups: '{n} backups', imported: '{n} files imported. The previous saves were copied to {b}.',
  backupDone: 'Backup in {b}', closeGameFirst: 'Close the game before importing saves.',
  confirmImport: "Import the saves from {emu}? They replace this instance's (a backup is made first).",
  started: 'Starting the game with {runner}…', notInstance: 'That folder has no install (pkg/ is missing).',
  removeConfirm: 'Remove {name} from the list? The folder stays on disk, nothing is deleted.',
  confirm: 'Confirm', ok: 'OK', error: 'Error: {e}',
  e_running: 'The game is already open.', 'e_no-wine': 'Wine was not found.', 'e_no-proton': 'Proton was not found (DLSS needs it).',
  'e_no-dlss': 'The DLSS files are missing.', 'e_not-ready': 'The instance is not built.', e_building: 'A build is running.',
  'e_not-instance': 'That folder has no install (pkg/ is missing).',
  noReleases: 'No versions in this channel.', localZip: 'Local .zip: {name}',
  wineMissing: 'Wine is missing', wineMissingBody: "The game is a Windows program and needs Wine. The launcher downloads its own Wine 11.22 (about 98 MB, no password, nothing is installed in the system). The first Set up downloads it by itself too.",
  installWine: 'Download Wine', wineInstalling: 'Downloading…', wineReady: 'Wine is ready. You can build and play now.',
  protonMissing: 'Proton is missing', protonMissingBody: 'DLSS runs through Proton. GE-Proton (about 570 MB) is downloaded into your user folder; no password needed.',
  getProton: 'Download GE-Proton', protonReady: 'GE-Proton is ready.', verifying: 'Verifying…',
  never: 'never',
  controls: 'Controls', controller: 'Controller', controllerHint: 'The game uses only this controller (handy when Steam or an adapter adds duplicate pads).',
  allControllers: 'All (automatic)', notConnected: 'not connected', detected: 'Connected controllers: {n}', noPads: 'No controller detected.', rescan: 'Scan again',
  inGameMenu: "The port also has its own in-game menu (Graphics, Controls, Audio and Game tabs): that is where buttons are remapped and the upscaler and picture are tuned.",
  yourScreen: 'your screen',
  safeRender: 'Safe drawing', safeRenderHint: "Slower. Turn it on only if the launcher's window stays blank or glitches (some drivers). Applies when the launcher is reopened.",
  gameSettings: 'Game settings', launcherSettings: 'Launcher settings', restartNowShort: 'The game is open: changes need a restart.',
  gameSettingsWhere: "The game's settings (resolution, upscaler, controller, language) are on the main page, under your instances.",
  kind_nup: '.nup package', kind_folder: 'Folder', kind_exefs: 'exefs folder', kind_dump: 'Dump (base + updates)', kind_none: '?',
};

const DICTS = { es, en };
let current = 'en';

export function setLang(l) { current = DICTS[l] ? l : 'en'; document.documentElement.lang = current === 'es' ? 'es-419' : 'en'; }
export function lang() { return current; }
export function t(key, vars = {}) {
  let s = DICTS[current][key] ?? DICTS.en[key] ?? DICTS.es[key] ?? key;
  for (const [k, v] of Object.entries(vars)) s = s.replaceAll(`{${k}}`, v);
  return s;
}
/** Error codes from the backend ("running", "no-proton") or plain messages. */
export function err(e) {
  const code = String(e);
  return DICTS[current]['e_' + code] ? t('e_' + code) : t('error', { e: code });
}
export function fmtDate(secs) {
  if (!secs) return t('never');
  return new Date(secs * 1000).toLocaleDateString(current === 'es' ? 'es-419' : 'en-US', { day: 'numeric', month: 'short' });
}
export function fmtSize(b) {
  if (!b) return '0 B';
  const u = ['B', 'KB', 'MB', 'GB', 'TB']; let i = 0;
  while (b >= 1000 && i < u.length - 1) { b /= 1000; i++; }
  return `${b.toFixed(i >= 3 ? 1 : 0)} ${u[i]}`;
}

# The Aegis’s Oppai · XC2S2 Edition — v0.1.21

- **Mods page:** a new **Mods** card lists the folders in `pkg/mods`, with a switch per mod (the runtime's own
  `[mods] disabled` in `nxrt.ini`), **Add mod** from a folder or a `.zip` (an extra wrapping folder is unwrapped;
  emulator layouts such as `atmosphere/contents/0100E95004038000/romfs` are accepted), **Remove**, and **Open folder**.
- **Mods that replace files inside the game's archive work without the port's Python tools:** most of the game's files
  are inside `bf2.ard`, and the runtime only serves a replacement for those from records prepared beforehand
  (`tools/mods/ard_pack.py`). The launcher now builds them itself — `.ard/<path>.rec` when the file still fits its slot,
  otherwise `.ard/append.bin` plus a repointed `romfs/bf2.arh` — when a mod is added, and again before a start when a
  mod's files or the game's archive index changed. Code patches (`exefs`) and cheats are listed but not applied.
- Changes take effect the next time the game starts.

---

**Español:** nueva tarjeta **Mods**: activar o desactivar cada mod, agregar mods desde una carpeta o un `.zip`, quitarlos
y abrir la carpeta. El lanzador prepara solo los mods que reemplazan archivos dentro del archivo grande del juego (antes
hacía falta una herramienta aparte). Los cambios se aplican la próxima vez que abras el juego.

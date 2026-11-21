# The Aegis’s Oppai · XC2S2 Edition — v0.1.5

**Set up from a `.nup` now works end to end** (verified: 15 minutes, 12,137 shaders, under 8 GB of memory). Builds that
failed with v0.1.2 or the v0.1.3/v0.1.4 test builds should be retried with this one.

- **Its own Wine 11.22**, downloaded once into the launcher's folder (~98 MB, SHA-256 checked, no password, nothing installed
  in the system): the same Wine on every distribution. Ubuntu/Kubuntu's older Wine is no longer used for building.
- **No Wine window at any time**: the prefix is prepared without a display; the Mono/Gecko prompts and the crash window are off.
- **Fixed: the shader step crashed** (`spirv_harness.exe` "has encountered a serious problem"): the launcher had disabled
  Wine's `mscoree`, which the port's .NET tools need to load their DLLs.
- **Fixed: the PC ran out of memory while unpacking the `.nup`** (the port's own extractor held it in memory, over 20 GB;
  the system then closed programs). The launcher now unpacks it natively, file by file.
- **Fixed: a build interrupted while unpacking** left the game files half extracted and the next try failed; they are now
  extracted again.
- v0.1.3 and v0.1.4 were test builds and are superseded by this release.

Set up needs about 45 GB free.

---

**Español:** la compilación desde el `.nup` ya funciona de punta a punta. El launcher trae su propio Wine 11.22 (sin
contraseña), no muestra ninguna ventana de Wine, arregla el cierre de la herramienta de shaders y el problema de memoria al
extraer el `.nup`. Si te falló antes, actualiza y vuelve a intentar.

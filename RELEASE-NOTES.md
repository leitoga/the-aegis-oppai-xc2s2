# The Aegis’s Oppai · XC2S2 Edition — v0.1.17

- **Controllers with DLSS (Proton) on systems without SDL3:** Proton reads controllers through SDL2, which current
  distributions provide as *sdl2-compat* on top of SDL3. Where SDL3 itself was missing, Proton printed
  `Failed loading SDL3 library.` and the game saw no controller at all. The launcher now carries its own SDL3 3.4.20
  (built from the official source, zlib license) and hands it to Proton, so nothing has to be installed.

---

**Español:** con DLSS (Proton), en sistemas sin SDL3 el juego no veía ningún mando («Failed loading SDL3 library.»).
Ahora el programa trae su propia SDL3 y se la pasa a Proton; no hace falta instalar nada.

# The Aegis’s Oppai · XC2S2 Edition — v0.1.20

- **DLSS stays on:** since the port's 0.2.4 the runtime keeps copies of the game's memory in video memory (up to a
  quarter of it, 2 GiB on an 8 GiB card). Together with what the desktop already uses, DLSS ran out of video memory
  about 26 s into the game, switched itself off, and the rest of the session fell back to the built-in upscaler at
  960x540 (a blurry picture). With DLSS chosen, the launcher now caps those copies at 1 GiB
  (`[nvn] pool_mirror_mb = 1024`); a value written in `nxrt.ini` takes precedence.

---

**Español:** con DLSS elegido, el juego ya no se queda sin memoria de video: antes, a los ~26 segundos, DLSS se apagaba
solo y la imagen quedaba borrosa (960x540). Ahora el lanzador limita a 1 GB las copias que el motor guarda en la placa.
Si algún parpadeo vuelve, se puede poner otro valor en `nxrt.ini` (`pool_mirror_mb`).

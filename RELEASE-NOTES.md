# The Aegis’s Oppai · XC2S2 Edition — v0.1.6

Fixes from the first test on a second PC (Kubuntu 26.04):

- **DLC**: the add-on content in the `.nup` (Torna – The Golden Country and the expansion pass, 7 titles) is now linked where
  the game looks for it. Without it a save that uses them said *"the downloadable content has been deleted"*.
- **Game files seen by Proton**: the port's setup engine links the game files with Windows junctions, which under Wine show up
  as entries ending in "?" and only Wine reads. The launcher now turns them into plain Linux links (exefs, romfs, game/romfs-301,
  dlc, game/dlc-301). An instance built with v0.1.5 is repaired by itself the next time you press Play — no need to rebuild.
- **Proton did not start** (`pfx.lock: FileNotFoundError`): the instance's `proton-pfx` folder is now created first.
- **When the game closes while starting**, the launcher now shows it, with the last lines of its log, instead of staying on
  "starting".

---

**Español:** arregla los DLC (Torna y el pase de expansión; tu partida decía «el contenido descargable se ha borrado»), los
enlaces de los archivos del juego para que Proton los vea, el arranque con Proton (`pfx.lock`) y muestra el error si el juego
se cierra al arrancar. Si ya compilaste con la v0.1.5, no hace falta recompilar: se arregla solo al apretar Jugar.

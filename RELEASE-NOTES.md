# The Aegis’s Oppai · XC2S2 Edition — v0.1.22

- **Mods that replace files inside the game's archive now take effect.** v0.1.21 prepared a file that still fit its
  original slot as an in-place record (`.ard/<path>.rec`, the port's `ard_pack.py` layout), but the runtime looks such a
  record up by the short name in the archive's own header (`bl000201.wimdo`), not by its path, so it was never used:
  a model kept its original definition while its appended texture was the mod's. Every replaced file now goes after
  the archive with its index entry repointed, which needs no name to match. Mods prepared by v0.1.21 are prepared again
  by themselves at the next start.

---

**Español:** los mods que cambian modelos ahora sí se aplican. La 0.1.21 preparaba parte de los archivos de una forma que
el motor no encontraba (por eso Mythra seguía con el traje de siempre). Los mods ya agregados se vuelven a preparar solos
la próxima vez que abras el juego.

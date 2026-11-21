# The Aegis’s Oppai · XC2S2 Edition — v0.1.2

**Fix: Set up stopped at the start with `OSError: [Errno 22] Invalid argument`** when the dump's path had a character outside
ASCII — the official `.nup` names carry an en dash ("Xenoblade Chronicles 2 **–** Nintendo Switch 2 Edition"). The port's
Python, running under some Wine versions, could not write that character to its log.

- The setup engine now runs in UTF-8 mode.
- When the `.nup` (or dump folder) path has such characters, the launcher gives the engine a plain-named link inside the
  instance (`source.nup`); the original file is not copied or changed.

If Set up stopped like this, update and press **Retry**: it continues in the same folder.

Note: Set up extracts the game's files from the `.nup` into the instance, so plan about 40 GB of free space for it.

---

**Español:** arregla el error `OSError: [Errno 22]` al empezar a compilar cuando el nombre del `.nup` tiene un guion largo
(«–»). Si te pasó, actualiza y aprieta **Reintentar**. La compilación necesita unos 40 GB libres.

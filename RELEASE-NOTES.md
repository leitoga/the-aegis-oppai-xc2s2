# The Aegis’s Oppai · XC2S2 Edition — v0.1.8

- **Controller not working in game** although the launcher listed it: with TAAU or FSR 3.1 the game runs through the
  launcher's Wine, which reads controllers through the system's SDL2 library — not always installed on Ubuntu/Kubuntu. Wine's
  own Linux paths for controllers (evdev and hidraw) are now turned on in the game's prefix, so it no longer depends on it.
  An existing install gets the change by itself on the next Play.
- The controller keeps working when the game's window does not have the focus (Proton's console window can take it).
- The update check now also works in builds made locally.

If a specific controller is chosen in *Controls* and the game does not respond, set it back to *All (automatic)*.

---

**Español:** arregla el mando que el lanzador detecta pero el juego no (con TAAU o FSR 3.1, Wine ahora lee los mandos
directo de Linux, sin depender de SDL2); el mando sigue andando aunque la ventana del juego no tenga el foco. Si elegiste un
mando en Controles y no responde, vuelve a «Todos».

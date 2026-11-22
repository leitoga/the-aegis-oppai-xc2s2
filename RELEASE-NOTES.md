# The Aegis’s Oppai · XC2S2 Edition — v0.1.7

- **The game no longer closes by itself after ~17 seconds.** It was started from a short-lived helper thread, and Proton
  (through the Steam runtime) ends the game when the thread that started it goes away. It now starts from a thread that lives
  as long as the game.
- **Loading card inside the game**: while the game's window is still black (the GPU driver prepares its pipelines; the first
  time takes longer), a dark card in the middle of the screen shows *Preparing the graphics*, a percentage and a bar that
  fills up. It is drawn by MangoHud inside the game's image, so no window can cover it; it goes away when the game draws.
- **FPS counter**: just the number, in a small dark rounded box, in the corner you choose. The port's own counter cannot be
  used (the port turns its overlay off for this game), so it is drawn by MangoHud; if MangoHud is not installed, the launcher
  downloads the official release once (SHA-256 checked, no password). A `DISABLE_MANGOHUD` in your session no longer hides it.
- **Proton's console window** is minimized once the game draws (KDE Plasma), and the game gets the focus.

---

**Español:** el juego ya no se cierra solo a los ~17 segundos; durante la pantalla negra se ve una tarjeta de carga con
porcentaje y barra (dibujada por MangoHud adentro del juego); el contador de FPS muestra solo el número en la esquina que
elijas (si no tienes MangoHud, el lanzador lo baja solo); la consola de Proton se minimiza cuando el juego arranca.

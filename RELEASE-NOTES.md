# The Aegis’s Oppai · XC2S2 Edition — v0.1.19

- **Pre-rendered cutscenes (films):** under Wine/Proton the runtime's GPU film decoders are not available, so the launcher
  now sends films straight to the CPU decoder (`[media] hwdec = 0`). Films start cleanly; some can still stall and then
  break up for a while: that part is in the runtime's film player and is reported upstream
  (weile10000/xc2-ns2-recomp#35).

---

**Español:** los videos pregrabados ahora van directo al decodificador por procesador y arrancan bien. Algunos todavía se
pueden trabar: esa parte está en el reproductor del motor y se la reportamos a sus autores.

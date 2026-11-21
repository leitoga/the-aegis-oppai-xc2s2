# The Aegis’s Oppai · XC2S2 Edition — v0.1.1

**Nothing to install by hand any more.** Download the AppImage, open it, and the launcher offers what is missing:

- **Wine**: when it is not installed, *Install Wine* installs it (and gamemode) with your distribution's package manager —
  pacman (Arch, CachyOS, Manjaro), apt (Ubuntu, Kubuntu, Mint, Debian), dnf (Fedora) or zypper (openSUSE). Your password goes
  in the system's own password window; if there is none, a terminal opens with the `sudo` command. The launcher never sees it.
- **GE-Proton for DLSS**: if no Proton is found, the *NVIDIA DLSS* card downloads the latest GE-Proton (about 570 MB, SHA-512
  checked) into Steam's compatibility tools folder, or the launcher's own folder when Steam is not installed. No password.
  Proton now also runs without Steam.
- Already installed Wine or Proton is used as before.

Everything from v0.1.0 is unchanged: one AppImage for every distribution, builds from your own dump, Wine/Proton with real
DLSS, settings on the main page, saves import, English and Spanish. See the README's *Status* table for what is verified.

---

**Español:** ya no hay que instalar nada a mano. Si falta Wine, el launcher lo instala con el gestor de paquetes de tu distro
(escribes la contraseña en la ventana del sistema o en una terminal); para DLSS baja GE-Proton solo, sin contraseña.

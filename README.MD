![Aurora Logo](./img/logo.svg)

<p align="right">
<a href="https://discord.gg/ecKbpAPW5T">
<img src="https://discordapp.com/api/guilds/1135362291311849693/widget.png?style=shield">
</a>
</p>

# AURORA — Tactical Radar System

**Aurora** is a next-generation open-source external Counter-Strike 2 read-only kernel-level gameplay enhancer, reborn with a signature **crimson dark-red interface** and refined visual experience.

- `Aurora` — dawn red light; the new name and face of this project
- `open source` — every component is auditable and here for you to learn from
- `external` — zero DLL injection into the target process
- `read-only` — absolutely no writes to CS2 process memory, maximising stealth
- `kernel-level` — bypasses user-mode WinAPIs to retrieve game state directly

This project is both a serious engineering exploration of the Windows kernel in [Rust](https://www.rust-lang.org) and a polished tactical radar you can actually use.

---

## ⚠️ WARNING

Aurora is **not** a plug-and-play cheat. It is built for stealth, which means setup takes a few deliberate steps. Please read the [Getting Started guide](https://wiki.valth.run/getting-started/) carefully and troubleshoot on your own before asking for help.

---

## ✨ Features

Because Aurora is read-only, skin changers and memory-writing cheats are out of scope. The feature set is tuned for competitive awareness:

- **🔴 External Radar**
  Broadcast your match to teammates on your own radar web page — no client needed for viewers.
- **🎯 Player ESP**
  Skeletons, 3D boxes, 2D boxes — fully configurable colours for enemies vs teammates, health bars, weapon names, and more.
- **💣 Bomb Info**
  Live detonation timer, defuse timer, and bomb-site indicator, all in the signature crimson glow.
- **🎮 Trigger Bot**
  Automatic fire when a target enters your crosshair.
- **👁️ Spectator List**
  See who is spectating you and who your observer target is.
- **🎥 Stream-Proof by Default**
  The overlay never appears in screen captures or streaming software.

Press `PAUSE` to open the in-overlay settings panel.

---

## 🎨 The Aurora Look

Aurora ships with a hand-crafted dark-red theme out of the box:

- Deep crimson backgrounds (`#0a0000` → `#8b0000`) with radial aurora glow
- Glowing red accent (`#ff1744`) on all interactive elements
- Monochromatic team highlights (icy blue for CT, crimson for T, glowing red for you)
- Frosted-glass panels with red-tinted borders and soft shadows
- Subtle grid overlay + pulsing aurora ambience on the landing page

Planned:
- Aimbot
- Competitive rank / win stats
- Additional Aurora-exclusive visual themes

---

## 🚀 Getting Started

Follow the official guide at:
<https://wiki.valth.run/getting-started/>

To build the radar web UI locally:

```bash
cd radar/web
yarn install
yarn start:dev    # dev server with hot reload
yarn build        # production build
```

---

## 🛠️ Troubleshooting

If the driver won't map or the controller won't start, check the [Troubleshooting section](https://wiki.valth.run/category/troubleshooting) of the wiki first. Most issues are documented there. For anything else, the Discord linked at the top of this README is the fastest way to get help.

---

## 🛡️ VAC

The same precautions described in [this write-up](https://github.com/dretax/GarHal_CSGO#starting-driver) have been adopted (with improvements of our own), including obfuscated identifiers and xor-encrypted strings to make detection more difficult. That said, no public tool is 100% undetectable. The original author has used a similar C-based driver/overlay for CS:GO for years without a VAC ban — but overwatch is still a real risk. Use at your own discretion.

---

## 📸 Screenshots

![](./img/showcase_01.png)
![](./img/showcase_02.png)

---

## 💬 Help

Join the official Discord for support and updates:

[![Discord Shield](https://discordapp.com/api/guilds/1135362291311849693/widget.png?style=shield)](https://discord.gg/ecKbpAPW5T)

---

<p align="center">
<sub><i>Aurora — Precision Vision, Red Theme Edition.</i></sub>
</p>

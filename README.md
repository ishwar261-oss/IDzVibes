# IDzVibes

> **Make Every Keystroke Feel Alive.**

IDzVibes is an ultra-low-latency keyboard sound engine that makes any keyboard feel and sound like a premium mechanical keyboard. Built with Rust for performance-critical audio processing and featuring an interactive dashboard.

[![Live Web Demo](https://img.shields.io/badge/🌐_Live_Demo-Try_in_Browser-6366f1?style=for-the-badge)](https://ishwar261-oss.github.io/IDzVibes/)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg?style=for-the-badge)](LICENSE)

---

### 🌐 Live Web Demo (Try It Instantly!)

You can try IDzVibes directly in your browser without installing anything!

👉 **[Launch Live Web Demo](https://ishwar261-oss.github.io/IDzVibes/)**

*Experience mechanical keyboard soundpacks (Creamy Thock, Cherry MX Red, Blue Clicky, Steelseries Apex Pro, Razer Green) live with Web Audio acoustic synthesis and interactive key visualizer.*

---

## ✨ Features

- ⚡ **Ultra-low latency** — Sub-10ms keypress-to-sound pipeline
- 🔊 **No wake-up delay** — Persistent audio stream, instant response even after minutes of idle
- 🎹 **9 Custom soundpacks** — Built-in support for Creamy Thock, Steelseries, HyperX, Razer Green, Cherry MX, eg-oreo & Mechvibes profiles
- 🔕 **Key-repeat deduplication** — Single crisp acoustic trigger on key press, no sound repetition when holding keys
- 🎛️ **Per-key configuration** — Individual sound assignment, volume, and pitch variation
- 🖱️ **Mouse sounds** — Click and scroll sound support
- 📊 **Performance diagnostics** — Real-time latency monitoring
- 🎨 **Modern Aptos Mono UI** — Dark-first, glassmorphism design with soundpack grid cards
- 🔒 **Privacy-first** — No keystroke logging, fully offline-capable

## 🏗️ Architecture

```
Keyboard Event → Windows Raw Input Thread → Lock-free Queue → Audio Scheduler → Voice Pool → WASAPI Output
```

The audio pipeline runs entirely in Rust, independent of the UI. The dashboard is a controller/monitor — never in the critical audio path.

## 🛠️ Tech Stack

| Component | Technology |
|-----------|-----------|
| Core Engine | Rust (Edition 2021) |
| Audio I/O | CPAL (WASAPI / DirectSound) |
| Audio Decoder | Symphonia (WAV / MP3 / FLAC / OGG) |
| Input Engine | Windows Raw Input API |
| Desktop UI | HTML5 + CSS Glassmorphism + Aptos Mono |
| Web Demo | Web Audio API Procedural Synthesis |

## 📦 Building & Running

### Prerequisites

- [Rust](https://rustup.rs/) (1.80+)
- Windows 10/11

### Run Desktop App

```bash
# Run release engine with low-latency WASAPI output & Web UI Dashboard
cargo run --release -p idz-desktop
```

Open `http://localhost:8420` in your browser to view the interactive dashboard.

### Run Workspace Tests

```bash
cargo test --workspace
```

## 📁 Project Structure

```
IDzVibes/
├── crates/
│   ├── idz-shared/      # Shared types (key codes, errors)
│   ├── idz-audio/       # Audio engine (voice pool, mixer, CPAL)
│   └── idz-input/       # Windows Raw Input keyboard listener
├── apps/
│   └── desktop/         # Desktop application binary & UI dashboard
├── soundpacks/          # Bundled soundpacks (Thock, Apex, Razer, HyperX)
├── docs/                # Documentation & GitHub Pages Web Demo
└── index.html           # Live Web Demo root file
```

## 📄 License

MIT — see [LICENSE](LICENSE)

## 🔒 Privacy

IDzVibes uses keyboard events **only** to trigger local audio playback. No keystroke content is ever stored, uploaded, or transmitted.

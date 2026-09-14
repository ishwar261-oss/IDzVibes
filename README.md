# IDzVibes

> **Make Every Keystroke Feel Alive.**

IDzVibes is an ultra-low-latency keyboard sound engine that makes any keyboard feel and sound like a premium mechanical keyboard. Built with Rust for performance-critical audio processing and Tauri for a modern desktop UI.

## ✨ Features

- ⚡ **Ultra-low latency** — Sub-10ms keypress-to-sound pipeline
- 🔊 **No wake-up delay** — Persistent audio stream, instant response even after minutes of idle
- 🎹 **Custom soundpacks** — Create, import, and share `.idzpack` sound profiles
- 🎛️ **Per-key configuration** — Individual sound assignment, volume, and pitch variation
- 🖱️ **Mouse sounds** — Click and scroll sound support
- 🌧️ **Ambient sounds** — Background audio (rain, café, office)
- 📊 **Performance diagnostics** — Real-time latency monitoring
- 🎨 **Modern UI** — Dark-first, premium design with Tauri + React
- 🔒 **Privacy-first** — No keystroke logging, fully offline-capable

## 🏗️ Architecture

```
Keyboard Event → Raw Input Thread → SPSC Ring Buffer → Audio Scheduler → Voice Pool → Mixer → Output
```

The audio pipeline runs entirely in Rust, independent of the UI. The React frontend is a controller/monitor — never in the critical audio path.

## 🛠️ Tech Stack

| Component | Technology |
|-----------|-----------|
| Core Engine | Rust |
| Audio I/O | CPAL (WASAPI) |
| Audio Decoder | Symphonia |
| Input | Windows Raw Input API |
| Desktop UI | Tauri 2.0 + React + TypeScript |
| Lock-free Queue | Custom SPSC Ring Buffer |

## 📦 Building

### Prerequisites

- [Rust](https://rustup.rs/) (1.80+)
- [Node.js](https://nodejs.org/) (18+)
- Windows 10/11

### Development

```bash
# Build and run the core engine (Phase 1 - console mode)
cargo run -p idz-desktop

# Build all crates
cargo build --workspace

# Run tests
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
│   └── desktop/         # Desktop application binary
├── soundpacks/          # Bundled soundpacks
└── docs/                # Documentation
```

## 📄 License

MIT — see [LICENSE](LICENSE)

## 🔒 Privacy

IDzVibes uses keyboard events **only** to trigger local audio playback. No keystroke content is ever stored, uploaded, or transmitted. Network access is used only for optional soundpack hub features.

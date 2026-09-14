# IDzVibes Architecture

IDzVibes is built with a decoupled, high-performance architecture where latency-sensitive audio generation is isolated from UI rendering and background tasks.

```
┌─────────────────────────────────────────────────────────────┐
│                      Operating System                       │
│  ┌───────────────────────┐       ┌───────────────────────┐  │
│  │   Windows Raw Input   │       │   WASAPI Audio Device │  │
│  └───────────┬───────────┘       └───────────▲───────────┘  │
└──────────────┼───────────────────────────────┼──────────────┘
               │ HWND Message Pump             │ Direct PCM DMA
               ▼                               │
┌─────────────────────────────┐    ┌───────────┴──────────────┐
│       idz-input Thread      │    │     CPAL Audio Thread    │
│  - Raw Input Registration   │    │  - Lock-Free SPSC Queue  │
│  - Scan code translation    │    │  - 32-Voice Preallocated │
│  - Zero heap allocation     │    │  - Linear Interpolation  │
└──────────────┬──────────────┘    │  - Soft-Knee Limiter     │
               │                   └───────────▲──────────────┘
               │ Lock-free                     │ Internal Arc
               │ Bounded SPSC                  │ Pre-decoded Float
               ▼                               │
┌──────────────────────────────────────────────┴──────────────┐
│                     idz-audio Engine                        │
│  - Persistent stream (eliminates DAC wake-up sleep delay)   │
│  - Key-down / Key-up sample selection with variance         │
│  - Independent mouse & ambient background channels          │
└─────────────────────────────────────────────────────────────┘
```

## Crate Responsibilities

1. **`idz-shared`**: Common types (`KeyCode`, `IdzError`), scan code lookups, filesystem names, display names.
2. **`idz-audio`**: Ultra-low-latency sound engine, 32-voice pool, linear interpolation resampler, ambient channel, soft-knee limiter.
3. **`idz-input`**: Dedicated background thread registering `RIDEV_INPUTSINK` for keyboard and mouse via Windows Raw Input.
4. **`apps/desktop`**: Application runner, diagnostics telemetry monitor, and future Tauri UI interface.

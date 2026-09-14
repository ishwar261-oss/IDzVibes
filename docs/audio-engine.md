# IDzVibes Audio Engine (`idz-audio`)

The audio engine is the performance-critical core of IDzVibes.

## Key Design Principles

1. **Persistent Output Stream**:
   The audio stream is initialized once at startup and kept running continuously. When idle, silence (0.0f) is fed to the DAC. This guarantees that your audio hardware and OS power-saving routines never put the endpoint into a sleep/power-save state, solving the "first key wake-up delay" completely.

2. **Zero-Allocation Hot Path**:
   All samples are decoded into memory as `f32` PCM slices during soundpack load. Triggering a voice involves assigning an existing slice reference and setting a cursor index.

3. **Lock-Free SPSC Command Queue**:
   Keystrokes are delivered to the audio callback thread using a bounded lock-free channel (`crossbeam-channel`). The audio thread never blocks on locks, heap allocations, or disk reads.

4. **Voice Stealing**:
   The pool maintains 32 concurrent voices. If all voices are busy, the oldest active voice is reassigned to the new note, ensuring instant response without memory explosion.

5. **Soft-Knee Limiter**:
   Fast typing (100+ keystrokes/sec) produces many overlapping sound waves. The soft-knee limiter uses a smooth exponential curve to prevent digital clipping while preserving transient snap.

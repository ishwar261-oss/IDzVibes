# IDzVibes Input Engine (`idz-input`)

## Native Windows Raw Input

`idz-input` utilizes the native Windows Raw Input model rather than legacy global Windows hooks (`SetWindowsHookEx`) or UI event listeners.

### Advantages of Raw Input
- **Sub-millisecond latency**: Direct hardware report packets from the kernel HID driver.
- **Background capture**: Configured with `RIDEV_INPUTSINK`, capturing global key events regardless of window focus.
- **Physical Device ID**: Distinguishes between multiple connected keyboards and mice.
- **No blocking**: Runs on a dedicated OS thread with an isolated `HWND_MESSAGE` window loop.

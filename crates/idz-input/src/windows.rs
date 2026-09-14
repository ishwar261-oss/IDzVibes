//! Windows Raw Input listener using a dedicated thread and message-only window.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::Instant;
use crossbeam_channel::{bounded, Receiver, Sender};
use tracing::info;

use idz_shared::error::InputError;
use idz_shared::KeyCode;
use crate::event::{InputEvent, InputEventKind};

use windows::core::w;
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Input::{
    GetRawInputData, RegisterRawInputDevices, HRAWINPUT, RAWINPUT, RAWINPUTDEVICE,
    RAWINPUTHEADER, RIDEV_INPUTSINK, RID_INPUT,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, GetMessageW,
    PostMessageW, PostQuitMessage, RegisterClassW, MSG, WINDOW_EX_STYLE,
    WNDCLASSW, WS_OVERLAPPED,
};

// Raw Input Keyboard Flags
const RI_KEY_BREAK: u16 = 0x0001;
const RI_KEY_E0: u16 = 0x0002;

// Raw Input Mouse Flags
const RI_MOUSE_LEFT_BUTTON_DOWN: u16 = 0x0001;
const RI_MOUSE_LEFT_BUTTON_UP: u16 = 0x0002;
const RI_MOUSE_RIGHT_BUTTON_DOWN: u16 = 0x0004;
const RI_MOUSE_RIGHT_BUTTON_UP: u16 = 0x0008;
const RI_MOUSE_MIDDLE_BUTTON_DOWN: u16 = 0x0010;
const RI_MOUSE_MIDDLE_BUTTON_UP: u16 = 0x0020;
const RI_MOUSE_WHEEL: u16 = 0x0400;

const WM_STOP_PUMP: u32 = 0x0400 + 101; // WM_USER + 101

use std::sync::OnceLock;

struct WindowState {
    event_tx: Sender<InputEvent>,
    capture_mouse: bool,
}

static GLOBAL_STATE: OnceLock<WindowState> = OnceLock::new();

pub struct WindowsInputEngine {
    event_rx: Receiver<InputEvent>,
    thread_handle: Option<JoinHandle<()>>,
    hwnd_val: isize,
    is_running: Arc<AtomicBool>,
}

impl WindowsInputEngine {
    /// Start the Windows Raw Input listener thread.
    pub fn new(capture_mouse: bool) -> Result<Self, InputError> {
        let (tx, rx) = bounded(256);
        let is_running = Arc::new(AtomicBool::new(true));
        let is_running_clone = Arc::clone(&is_running);

        let (init_tx, init_rx) = crossbeam_channel::bounded::<Result<isize, String>>(1);

        let thread_handle = thread::Builder::new()
            .name("idz-raw-input".to_string())
            .spawn(move || {
                let hwnd = match create_raw_input_window(tx, capture_mouse) {
                    Ok(h) => {
                        let _ = init_tx.send(Ok(h.0 as isize));
                        h
                    }
                    Err(e) => {
                        let _ = init_tx.send(Err(e.to_string()));
                        return;
                    }
                };

                // Message pump loop
                unsafe {
                    let mut msg = MSG::default();
                    while GetMessageW(&mut msg, None, 0, 0).as_bool() {
                        if msg.message == WM_STOP_PUMP {
                            break;
                        }
                        DispatchMessageW(&msg);
                    }

                    let _ = DestroyWindow(hwnd);
                }

                is_running_clone.store(false, Ordering::SeqCst);
                info!("Windows Raw Input thread exited cleanly");
            })
            .map_err(|e| InputError::Thread(e.to_string()))?;

        let hwnd_val = init_rx
            .recv()
            .map_err(|e| InputError::Thread(format!("Failed to receive init HWND: {e}")))?
            .map_err(InputError::WindowCreation)?;

        Ok(Self {
            event_rx: rx,
            thread_handle: Some(thread_handle),
            hwnd_val,
            is_running,
        })
    }

    /// Event receiver for input events.
    pub fn receiver(&self) -> Receiver<InputEvent> {
        self.event_rx.clone()
    }

    pub fn is_running(&self) -> bool {
        self.is_running.load(Ordering::Relaxed)
    }

    /// Stop the input engine and wait for thread termination.
    pub fn stop(&mut self) {
        if self.hwnd_val != 0 {
            unsafe {
                let hwnd = HWND(self.hwnd_val as *mut _);
                let _ = PostMessageW(hwnd, WM_STOP_PUMP, WPARAM(0), LPARAM(0));
            }
        }
        if let Some(handle) = self.thread_handle.take() {
            let _ = handle.join();
        }
    }
}

impl Drop for WindowsInputEngine {
    fn drop(&mut self) {
        self.stop();
    }
}

fn create_raw_input_window(
    tx: Sender<InputEvent>,
    capture_mouse: bool,
) -> Result<HWND, InputError> {
    unsafe {
        let class_name = w!("IDzVibesRawInputClass");
        let h_instance = GetModuleHandleW(None)
            .map_err(|e| InputError::WinApi(format!("GetModuleHandleW failed: {e}")))?;

        let wnd_class = WNDCLASSW {
            lpfnWndProc: Some(raw_input_wnd_proc),
            hInstance: h_instance.into(),
            lpszClassName: class_name,
            ..Default::default()
        };

        RegisterClassW(&wnd_class);

        // Store global state for wndproc
        let _ = GLOBAL_STATE.set(WindowState {
            event_tx: tx,
            capture_mouse,
        });

        // Message-only window (HWND_MESSAGE = -3 as isize)
        let hwnd_message = HWND(-3isize as *mut _);
        let hwnd = CreateWindowExW(
            WINDOW_EX_STYLE(0),
            class_name,
            w!("IDzVibesRawInput"),
            WS_OVERLAPPED,
            0,
            0,
            0,
            0,
            hwnd_message,
            None,
            h_instance,
            None,
        ).map_err(|e| InputError::WindowCreation(format!("CreateWindowExW failed: {e}")))?;

        // Register Raw Input devices for Keyboard (UsagePage 0x01, Usage 0x06)
        let mut devices = vec![RAWINPUTDEVICE {
            usUsagePage: 0x01, // Generic Desktop
            usUsage: 0x06,     // Keyboard
            dwFlags: RIDEV_INPUTSINK,
            hwndTarget: hwnd,
        }];

        if capture_mouse {
            devices.push(RAWINPUTDEVICE {
                usUsagePage: 0x01, // Generic Desktop
                usUsage: 0x02,     // Mouse
                dwFlags: RIDEV_INPUTSINK,
                hwndTarget: hwnd,
            });
        }

        RegisterRawInputDevices(
            &devices,
            std::mem::size_of::<RAWINPUTDEVICE>() as u32,
        ).map_err(|e| InputError::Registration(format!("RegisterRawInputDevices failed: {e}")))?;

        info!("Registered Windows Raw Input for keyboard (and mouse: {capture_mouse})");
        Ok(hwnd)
    }
}

unsafe extern "system" fn raw_input_wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        windows::Win32::UI::WindowsAndMessaging::WM_INPUT => {
            let mut raw_input = RAWINPUT::default();
            let mut size = std::mem::size_of::<RAWINPUT>() as u32;

            let bytes_copied = GetRawInputData(
                HRAWINPUT(lparam.0 as *mut _),
                RID_INPUT,
                Some(&mut raw_input as *mut _ as *mut _),
                &mut size,
                std::mem::size_of::<RAWINPUTHEADER>() as u32,
            );

            if bytes_copied != u32::MAX {
                let now_us = Instant::now().elapsed().as_micros() as u64;

                if let Some(state) = GLOBAL_STATE.get() {
                    let header = raw_input.header;
                    let device_id = header.hDevice.0 as usize;

                    if header.dwType == 1 {
                        // Keyboard event
                        let kb = raw_input.data.keyboard;
                        let scan_code = kb.MakeCode as u32;
                        let flags = kb.Flags;

                        // Ignore overrun / key-burst pseudo codes
                        if scan_code != 0 && scan_code != 0xFF {
                            let is_break = (flags & RI_KEY_BREAK) != 0;
                            let is_extended = (flags & RI_KEY_E0) != 0;

                            let key = KeyCode::from_scan_code(scan_code, is_extended);
                            let kind = if is_break {
                                InputEventKind::KeyUp
                            } else {
                                InputEventKind::KeyDown
                            };

                            let _ = state.event_tx.try_send(InputEvent {
                                key,
                                kind,
                                timestamp_us: now_us,
                                device_id,
                            });
                        }
                    } else if header.dwType == 0 && state.capture_mouse {
                        // Mouse event
                        let mouse = raw_input.data.mouse;
                        let btn_flags = mouse.Anonymous.Anonymous.usButtonFlags;

                        if (btn_flags & RI_MOUSE_LEFT_BUTTON_DOWN) != 0 {
                            let _ = state.event_tx.try_send(InputEvent {
                                key: KeyCode::MouseLeft,
                                kind: InputEventKind::MouseDown,
                                timestamp_us: now_us,
                                device_id,
                            });
                        } else if (btn_flags & RI_MOUSE_LEFT_BUTTON_UP) != 0 {
                            let _ = state.event_tx.try_send(InputEvent {
                                key: KeyCode::MouseLeft,
                                kind: InputEventKind::MouseUp,
                                timestamp_us: now_us,
                                device_id,
                            });
                        }

                        if (btn_flags & RI_MOUSE_RIGHT_BUTTON_DOWN) != 0 {
                            let _ = state.event_tx.try_send(InputEvent {
                                key: KeyCode::MouseRight,
                                kind: InputEventKind::MouseDown,
                                timestamp_us: now_us,
                                device_id,
                            });
                        } else if (btn_flags & RI_MOUSE_RIGHT_BUTTON_UP) != 0 {
                            let _ = state.event_tx.try_send(InputEvent {
                                key: KeyCode::MouseRight,
                                kind: InputEventKind::MouseUp,
                                timestamp_us: now_us,
                                device_id,
                            });
                        }

                        if (btn_flags & RI_MOUSE_MIDDLE_BUTTON_DOWN) != 0 {
                            let _ = state.event_tx.try_send(InputEvent {
                                key: KeyCode::MouseMiddle,
                                kind: InputEventKind::MouseDown,
                                timestamp_us: now_us,
                                device_id,
                            });
                        } else if (btn_flags & RI_MOUSE_MIDDLE_BUTTON_UP) != 0 {
                            let _ = state.event_tx.try_send(InputEvent {
                                key: KeyCode::MouseMiddle,
                                kind: InputEventKind::MouseUp,
                                timestamp_us: now_us,
                                device_id,
                            });
                        }

                        if (btn_flags & RI_MOUSE_WHEEL) != 0 {
                            let wheel_delta = mouse.Anonymous.Anonymous.usButtonData as i16;
                            let key = if wheel_delta > 0 {
                                KeyCode::MouseWheelUp
                            } else {
                                KeyCode::MouseWheelDown
                            };
                            let _ = state.event_tx.try_send(InputEvent {
                                key,
                                kind: InputEventKind::MouseDown,
                                timestamp_us: now_us,
                                device_id,
                            });
                        }
                    }
                }
            }

            DefWindowProcW(hwnd, msg, wparam, lparam)
        }
        windows::Win32::UI::WindowsAndMessaging::WM_DESTROY => {
            PostQuitMessage(0);
            LRESULT(0)
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

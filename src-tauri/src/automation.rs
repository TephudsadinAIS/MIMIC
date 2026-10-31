//! Deliberately narrow Windows automation proof for the controlled MIMIC demo.
//!
//! This module never accepts coordinates, arbitrary executable paths, or text to
//! type from the web UI.  The only action it can perform is advancing the local
//! demo window after re-checking its identity and expected visible control.

use serde::Serialize;
use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        Mutex, Once,
    },
    thread,
    time::Duration,
};
use windows::{
    core::{w, BOOL, PCWSTR},
    Win32::{
        Foundation::{HWND, LPARAM, LRESULT, WPARAM},
        System::LibraryLoader::GetModuleHandleW,
        UI::WindowsAndMessaging::{
            CreateWindowExW, DefWindowProcW, DispatchMessageW, EnumWindows, FindWindowExW,
            GetClassNameW, GetMessageW, GetWindowTextLengthW, GetWindowTextW,
            GetWindowThreadProcessId, IsWindow, IsWindowVisible, LoadCursorW, PostQuitMessage,
            RegisterClassW, SendMessageW, SetWindowTextW, ShowWindow, TranslateMessage,
            CREATESTRUCTW, CW_USEDEFAULT, GWLP_USERDATA, HMENU, IDC_ARROW, MSG, SW_SHOW,
            WINDOW_EX_STYLE, WM_COMMAND, WM_DESTROY, WM_NCCREATE, WNDCLASSW, WS_CHILD,
            WS_OVERLAPPEDWINDOW, WS_VISIBLE,
        },
    },
};

const DEMO_CLASS: PCWSTR = w!("MimicAutomationDemoTarget");
const DEMO_LOGIN: &str = "MIMIC Automation Demo - Login";
const DEMO_SERVER: &str = "MIMIC Automation Demo - Server selection";
const DEMO_READY: &str = "MIMIC Automation Demo - Ready";
const LOGIN_BUTTON: &str = "Continue to Server";
const SERVER_BUTTON: &str = "Select Test Server";
const DEMO_BUTTON_ID: usize = 101;

static DEMO_STARTED: AtomicBool = AtomicBool::new(false);
static EMERGENCY_STOP: AtomicBool = AtomicBool::new(false);
static REGISTER_CLASS: Once = Once::new();
static DEMO_WINDOW: Mutex<Option<isize>> = Mutex::new(None);

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WindowInfo {
    pub id: isize,
    pub title: String,
    pub process_id: u32,
    pub is_demo: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DemoRunResult {
    pub status: String,
    pub message: String,
    pub target_window_id: isize,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DemoObservation {
    pub state: String,
    pub title: String,
    pub target_window_id: isize,
}

fn to_wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(Some(0)).collect()
}

unsafe fn title_of(window: HWND) -> String {
    let length = GetWindowTextLengthW(window);
    if length <= 0 {
        return String::new();
    }
    let mut buffer = vec![0u16; (length + 1) as usize];
    let written = GetWindowTextW(window, &mut buffer);
    String::from_utf16_lossy(&buffer[..written as usize])
}

unsafe fn class_of(window: HWND) -> String {
    let mut buffer = [0u16; 256];
    let written = GetClassNameW(window, &mut buffer);
    String::from_utf16_lossy(&buffer[..written as usize])
}

unsafe extern "system" fn collect_windows(window: HWND, data: LPARAM) -> BOOL {
    if !IsWindowVisible(window).as_bool() {
        return BOOL(1);
    }
    let title = title_of(window);
    if title.trim().is_empty() {
        return BOOL(1);
    }
    let mut process_id = 0;
    GetWindowThreadProcessId(window, Some(&mut process_id));
    let windows = &mut *(data.0 as *mut Vec<WindowInfo>);
    let is_demo = class_of(window) == "MimicAutomationDemoTarget";
    windows.push(WindowInfo {
        id: window.0 as isize,
        is_demo,
        title,
        process_id,
    });
    BOOL(1)
}

pub fn list_windows() -> Result<Vec<WindowInfo>, String> {
    let mut windows: Vec<WindowInfo> = Vec::new();
    unsafe {
        EnumWindows(
            Some(collect_windows),
            LPARAM((&mut windows as *mut Vec<WindowInfo>) as isize),
        )
        .map_err(|error| format!("Unable to enumerate Windows windows: {error}"))?;
    }
    windows.sort_by(|left, right| left.title.cmp(&right.title));
    Ok(windows)
}

unsafe extern "system" fn demo_window_proc(
    window: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match message {
        WM_NCCREATE => {
            let creation = lparam.0 as *const CREATESTRUCTW;
            if !creation.is_null() {
                windows::Win32::UI::WindowsAndMessaging::SetWindowLongPtrW(
                    window,
                    GWLP_USERDATA,
                    (*creation).lpCreateParams as isize,
                );
            }
            DefWindowProcW(window, message, wparam, lparam)
        }
        WM_COMMAND if (wparam.0 & 0xffff) == DEMO_BUTTON_ID => {
            let stage =
                windows::Win32::UI::WindowsAndMessaging::GetWindowLongPtrW(window, GWLP_USERDATA);
            let button = HWND(lparam.0 as *mut _);
            if stage == 0 {
                let _ = SetWindowTextW(window, w!("MIMIC Automation Demo - Server selection"));
                let _ = SetWindowTextW(button, w!("Select Test Server"));
                windows::Win32::UI::WindowsAndMessaging::SetWindowLongPtrW(
                    window,
                    GWLP_USERDATA,
                    1,
                );
            } else if stage == 1 {
                let _ = SetWindowTextW(window, w!("MIMIC Automation Demo - Ready"));
                let _ = SetWindowTextW(button, w!("Demo completed"));
                windows::Win32::UI::WindowsAndMessaging::SetWindowLongPtrW(
                    window,
                    GWLP_USERDATA,
                    2,
                );
            }
            LRESULT(0)
        }
        WM_DESTROY => {
            if let Ok(mut demo_window) = DEMO_WINDOW.lock() {
                *demo_window = None;
            }
            DEMO_STARTED.store(false, Ordering::SeqCst);
            PostQuitMessage(0);
            LRESULT(0)
        }
        _ => DefWindowProcW(window, message, wparam, lparam),
    }
}

unsafe fn create_demo_window() -> Result<(), String> {
    REGISTER_CLASS.call_once(|| {
        let instance = GetModuleHandleW(None).unwrap_or_default();
        let cursor = LoadCursorW(None, IDC_ARROW).unwrap_or_default();
        let class = WNDCLASSW {
            hInstance: instance.into(),
            hCursor: cursor,
            lpszClassName: DEMO_CLASS,
            lpfnWndProc: Some(demo_window_proc),
            ..Default::default()
        };
        let _ = RegisterClassW(&class);
    });

    let instance = GetModuleHandleW(None).map_err(|error| error.to_string())?;
    let window = CreateWindowExW(
        WINDOW_EX_STYLE(0),
        DEMO_CLASS,
        w!("MIMIC Automation Demo - Login"),
        WS_OVERLAPPEDWINDOW | WS_VISIBLE,
        CW_USEDEFAULT,
        CW_USEDEFAULT,
        520,
        280,
        None,
        None,
        Some(instance.into()),
        None,
    )
    .map_err(|error| error.to_string())?;
    let button = CreateWindowExW(
        WINDOW_EX_STYLE(0),
        w!("BUTTON"),
        w!("Continue to Server"),
        WS_CHILD | WS_VISIBLE,
        155,
        130,
        190,
        42,
        Some(window),
        Some(HMENU(DEMO_BUTTON_ID as *mut _)),
        Some(instance.into()),
        None,
    )
    .map_err(|error| error.to_string())?;
    if button.0.is_null() {
        return Err("Unable to create the demo action button".into());
    }
    let _ = ShowWindow(window, SW_SHOW);
    *DEMO_WINDOW.lock().map_err(|_| "Demo window lock failed")? = Some(window.0 as isize);
    Ok(())
}

pub fn start_demo() -> Result<(), String> {
    if DEMO_STARTED.swap(true, Ordering::SeqCst) {
        return Ok(());
    }
    // Starting the controlled target is an explicit new-run gesture.  A prior
    // emergency stop remains fail-closed until this point.
    EMERGENCY_STOP.store(false, Ordering::SeqCst);
    thread::spawn(|| unsafe {
        if let Err(error) = create_demo_window() {
            eprintln!("MIMIC demo target failed: {error}");
            DEMO_STARTED.store(false, Ordering::SeqCst);
            return;
        }
        let mut message = MSG::default();
        while GetMessageW(&mut message, None, 0, 0).as_bool() {
            let _ = TranslateMessage(&message);
            DispatchMessageW(&message);
        }
    });
    for _ in 0..20 {
        if DEMO_WINDOW.lock().ok().and_then(|value| *value).is_some() {
            return Ok(());
        }
        thread::sleep(Duration::from_millis(50));
    }
    Err("Demo target did not become ready in time".into())
}

fn expected_transition_for(title: &str) -> Option<(&'static str, &'static str)> {
    match title {
        DEMO_LOGIN => Some((LOGIN_BUTTON, DEMO_SERVER)),
        DEMO_SERVER => Some((SERVER_BUTTON, DEMO_READY)),
        _ => None,
    }
}

fn state_for_title(title: &str) -> Option<&'static str> {
    match title {
        DEMO_LOGIN => Some("login"),
        DEMO_SERVER => Some("server"),
        DEMO_READY => Some("ready"),
        _ => None,
    }
}

pub fn observe_demo(target_window_id: isize) -> Result<DemoObservation, String> {
    let window = HWND(target_window_id as *mut _);
    unsafe {
        if !IsWindow(Some(window)).as_bool() || !IsWindowVisible(window).as_bool() {
            return Err("Selected target is no longer an open visible window".into());
        }
        if class_of(window) != "MimicAutomationDemoTarget" {
            return Err("Selected target is not the controlled MIMIC Demo window".into());
        }
        let title = title_of(window);
        let state = state_for_title(&title)
            .ok_or_else(|| "Demo window is in an unexpected state".to_string())?;
        Ok(DemoObservation {
            state: state.into(),
            title,
            target_window_id,
        })
    }
}

pub fn advance_demo(
    target_window_id: isize,
    permission_granted: bool,
) -> Result<DemoRunResult, String> {
    if !permission_granted {
        return Err("Automation permission was not granted for this run".into());
    }
    let window = HWND(target_window_id as *mut _);
    unsafe {
        if !IsWindow(Some(window)).as_bool() || !IsWindowVisible(window).as_bool() {
            return Err(
                "Selected target is no longer an open visible window; no action was taken".into(),
            );
        }
        let title = title_of(window);
        if class_of(window) != "MimicAutomationDemoTarget" {
            return Err(
                "Selected target is not the controlled MIMIC Demo window; no action was taken"
                    .into(),
            );
        }
        let (expected_button, expected_next_title) =
            expected_transition_for(&title).ok_or_else(|| {
                "Unexpected demo state. MIMIC stopped without clicking anything.".to_string()
            })?;
        let button_name = to_wide(expected_button);
        let button = FindWindowExW(
            Some(window),
            None,
            w!("BUTTON"),
            PCWSTR(button_name.as_ptr()),
        )
        .map_err(|error| format!("Unable to locate the expected control: {error}"))?;
        if button.0.is_null() {
            return Err(
                "Expected action control is missing. MIMIC stopped without clicking.".into(),
            );
        }
        if EMERGENCY_STOP.load(Ordering::SeqCst) {
            return Ok(DemoRunResult {
                status: "stopped".into(),
                message: "Emergency stop was requested before input".into(),
                target_window_id,
            });
        }
        // BM_CLICK is scoped to the verified child control. It never sends global mouse input.
        const BM_CLICK: u32 = 0x00F5;
        SendMessageW(button, BM_CLICK, Some(WPARAM(0)), Some(LPARAM(0)));
        for _ in 0..20 {
            if EMERGENCY_STOP.load(Ordering::SeqCst) {
                return Ok(DemoRunResult {
                    status: "stopped".into(),
                    message: "Emergency stop interrupted verification".into(),
                    target_window_id,
                });
            }
            let observed = title_of(window);
            if observed == expected_next_title {
                let status = if observed == DEMO_READY {
                    "completed"
                } else {
                    "verified"
                };
                return Ok(DemoRunResult {
                    status: status.into(),
                    message: format!("Verified expected state: {observed}"),
                    target_window_id,
                });
            }
            thread::sleep(Duration::from_millis(100));
        }
        Err("Action was sent but the expected state was not observed before timeout".into())
    }
}

pub fn emergency_stop() -> DemoRunResult {
    EMERGENCY_STOP.store(true, Ordering::SeqCst);
    DemoRunResult {
        status: "stopped".into(),
        message: "Emergency stop requested. No further demo input will be sent.".into(),
        target_window_id: 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn controlled_demo_observe_act_verify_and_fail_closed() {
        start_demo().expect("demo target should open");
        thread::sleep(Duration::from_millis(100));
        let demo = list_windows()
            .expect("window enumeration should work")
            .into_iter()
            .find(|window| window.is_demo && window.title == DEMO_LOGIN)
            .expect("demo login window should be discoverable");
        assert_eq!(
            observe_demo(demo.id)
                .expect("demo should be observable")
                .state,
            "login"
        );

        let first = advance_demo(demo.id, true).expect("login action should verify server state");
        assert_eq!(first.status, "verified");
        let second = advance_demo(demo.id, true).expect("server action should verify ready state");
        assert_eq!(second.status, "completed");
        assert!(
            advance_demo(demo.id, true).is_err(),
            "Ready is terminal and must not click again"
        );
        assert!(
            advance_demo(0, true).is_err(),
            "Missing target must fail before input"
        );
    }
}

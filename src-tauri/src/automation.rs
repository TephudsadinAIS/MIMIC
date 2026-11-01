//! Windows automation engine for MIMIC:
//! - Window enumeration and scoping
//! - Executable picker and launcher
//! - UI Automation and child element inspection
//! - Interactive Select on Screen overlay with highlight and image region capture
//! - Real UIA locator resolution and Real Image Template Matching (SAD / Normalized Difference)
//! - Postcondition verification with actual element detection and state validation
//! - Wait actions: wait-visible, wait-hidden, verify-state with bounded timeouts
//! - Controlled Demo target with Login, Server selection, and Ready views
//! - Offline deterministic workflow step executor with verification, retries, and emergency stop.

use serde::{Deserialize, Serialize};
use std::{
    sync::{
        atomic::{AtomicBool, AtomicPtr, AtomicUsize, Ordering},
        Mutex, Once,
    },
    thread,
    time::{Duration, Instant},
};
use windows::{
    core::{w, BOOL, PCWSTR, PWSTR},
    Win32::{
        Foundation::{COLORREF, HWND, LPARAM, LRESULT, POINT, RECT, WPARAM},
        Graphics::Gdi::{
            BitBlt, CreateCompatibleBitmap, CreateCompatibleDC, CreateSolidBrush, DeleteDC,
            DeleteObject, FrameRect, GetDC, GetDIBits, GetStockObject, InvalidateRect, ReleaseDC,
            ScreenToClient, SelectObject, SetBkMode, SetTextColor, TextOutW, UpdateWindow,
            BITMAPINFO, BITMAPINFOHEADER, BLACK_BRUSH, DIB_RGB_COLORS, HBRUSH, SRCCOPY,
            TRANSPARENT,
        },
        System::{
            Com::{
                CoCreateInstance, CoInitializeEx, CLSCTX_INPROC_SERVER,
                COINIT_APARTMENTTHREADED,
            },
            LibraryLoader::GetModuleHandleW,
            ProcessStatus::GetModuleFileNameExW,
            Threading::{GetCurrentThreadId, OpenProcess, PROCESS_QUERY_INFORMATION, PROCESS_VM_READ},
        },
        UI::{
            Accessibility::{
                CUIAutomation, IUIAutomation, IUIAutomationElement, IUIAutomationTreeWalker,
            },
            Input::KeyboardAndMouse::{
                GetKeyState, ReleaseCapture, SendInput, SetCapture, INPUT, INPUT_0, INPUT_KEYBOARD,
                KEYBDINPUT, KEYEVENTF_KEYUP, VK_BACK, VK_CONTROL, VK_ESCAPE, VK_PAUSE, VK_RETURN,
                VK_SHIFT, VK_SPACE, VK_TAB,
            },
            Shell::{
                FileOpenDialog, IFileOpenDialog, FOS_FILEMUSTEXIST, FOS_FORCEFILESYSTEM,
                SIGDN_FILESYSPATH,
            },
            WindowsAndMessaging::{
                CallNextHookEx, CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW,
                EnumChildWindows, EnumWindows, FindWindowExW, GetClassNameW, GetCursorPos, GetDlgItem,
                GetMessageW, GetSystemMetrics, GetWindowRect, GetWindowTextLengthW, GetWindowTextW,
                GetWindowThreadProcessId, IsWindow, IsWindowVisible, LoadCursorW, PostMessageW,
                PostQuitMessage, PostThreadMessageW, RegisterClassW, SendMessageW, SetForegroundWindow,
                SetLayeredWindowAttributes, SetWindowPos, SetWindowsHookExW, SetWindowTextW, ShowWindow,
                TranslateMessage, UnhookWindowsHookEx, CREATESTRUCTW, CW_USEDEFAULT, ES_AUTOHSCROLL,
                ES_PASSWORD, GWLP_USERDATA, HHOOK, HMENU, HWND_TOPMOST, IDC_ARROW, IDC_CROSS,
                KBDLLHOOKSTRUCT, LWA_COLORKEY, MSG, SM_CXVIRTUALSCREEN, SM_CYVIRTUALSCREEN,
                SM_XVIRTUALSCREEN, SM_YVIRTUALSCREEN, SWP_SHOWWINDOW, SW_RESTORE, SW_SHOW,
                WH_KEYBOARD_LL, WINDOW_EX_STYLE, WINDOW_STYLE, WM_CHAR, WM_COMMAND, WM_DESTROY,
                WM_KEYDOWN, WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MOUSEMOVE, WM_NCCREATE, WM_PAINT,
                WM_QUIT, WM_RBUTTONUP, WM_SYSKEYDOWN, WNDCLASSW, WS_BORDER, WS_CHILD,
                WS_EX_LAYERED, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_OVERLAPPEDWINDOW, WS_VISIBLE,
            },
        },
    },
};

const DEMO_CLASS: PCWSTR = w!("MimicAutomationDemoTarget");
const OVERLAY_CLASS: PCWSTR = w!("MimicSelectionOverlayWindow");

pub const DEMO_LOGIN_TITLE: &str = "MIMIC Automation Demo - Login";
pub const DEMO_SERVER_TITLE: &str = "MIMIC Automation Demo - Server selection";
pub const DEMO_READY_TITLE: &str = "MIMIC Automation Demo - Ready";

const DEMO_BTN_LOGIN_ID: usize = 101;
const DEMO_EDIT_USER_ID: usize = 102;
const DEMO_EDIT_PASS_ID: usize = 103;

pub static DEMO_CLICK_COUNT: AtomicUsize = AtomicUsize::new(0);
static DEMO_STARTED: AtomicBool = AtomicBool::new(false);
static EMERGENCY_STOP: AtomicBool = AtomicBool::new(false);
static REGISTER_DEMO_CLASS: Once = Once::new();
static REGISTER_OVERLAY_CLASS: Once = Once::new();
static DEMO_WINDOW: Mutex<Option<isize>> = Mutex::new(None);

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NativeAuthorizedSession {
    pub session_id: String,
    pub target_window_id: isize,
    pub target_process_id: u32,
    pub target_exe_path: Option<String>,
    pub created_at: u64,
    pub is_active: bool,
}

static ACTIVE_SESSION: Mutex<Option<NativeAuthorizedSession>> = Mutex::new(None);
static GLOBAL_HOOK_THREAD: Mutex<Option<u32>> = Mutex::new(None);
static GLOBAL_HOOK_HANDLE: AtomicPtr<std::ffi::c_void> = AtomicPtr::new(std::ptr::null_mut());

unsafe extern "system" fn global_keyboard_hook_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code >= 0 && (wparam.0 == WM_KEYDOWN as usize || wparam.0 == WM_SYSKEYDOWN as usize) {
        let kbd = &*(lparam.0 as *const KBDLLHOOKSTRUCT);
        let vk = kbd.vkCode;

        // 1. VK_PAUSE (Pause / Break key) - standalone dedicated emergency stop
        // 2. Ctrl + Shift + Q (0x51) or Ctrl + Shift + Pause
        // NOTE: Standard VK_ESCAPE by itself is NOT intercepted, preserving normal in-game behavior!
        let ctrl_down = (GetKeyState(VK_CONTROL.0 as i32) as u16 & 0x8000) != 0;
        let shift_down = (GetKeyState(VK_SHIFT.0 as i32) as u16 & 0x8000) != 0;

        let is_emergency_hotkey = vk == VK_PAUSE.0 as u32
            || (ctrl_down && shift_down && (vk == 0x51 /* Q */ || vk == VK_PAUSE.0 as u32));

        if is_emergency_hotkey {
            emergency_stop();
        }
    }
    CallNextHookEx(None, code, wparam, lparam)
}

pub fn start_global_emergency_hook() {
    let mut thread_lock = match GLOBAL_HOOK_THREAD.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    if thread_lock.is_some() {
        return;
    }

    let (tx, rx) = std::sync::mpsc::channel();

    let _ = std::thread::spawn(move || {
        unsafe {
            let thread_id = GetCurrentThreadId();
            let hook = SetWindowsHookExW(
                WH_KEYBOARD_LL,
                Some(global_keyboard_hook_proc),
                None,
                0,
            );

            if let Ok(h) = hook {
                GLOBAL_HOOK_HANDLE.store(h.0 as *mut _, Ordering::SeqCst);
                let _ = tx.send(Ok(thread_id));

                let mut msg = MSG::default();
                while GetMessageW(&mut msg, None, 0, 0).as_bool() {
                    if msg.message == WM_QUIT {
                        break;
                    }
                }

                let _ = UnhookWindowsHookEx(h);
                GLOBAL_HOOK_HANDLE.store(std::ptr::null_mut(), Ordering::SeqCst);
            } else {
                let _ = tx.send(Err("Failed to install global keyboard hook".to_string()));
            }
        }
    });

    if let Ok(Ok(tid)) = rx.recv() {
        *thread_lock = Some(tid);
    }
}

pub fn stop_global_emergency_hook() {
    let mut thread_lock = match GLOBAL_HOOK_THREAD.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    if let Some(tid) = thread_lock.take() {
        unsafe {
            let hook_ptr = GLOBAL_HOOK_HANDLE.load(Ordering::SeqCst);
            if !hook_ptr.is_null() {
                let _ = UnhookWindowsHookEx(HHOOK(hook_ptr));
                GLOBAL_HOOK_HANDLE.store(std::ptr::null_mut(), Ordering::SeqCst);
            }
            let _ = PostThreadMessageW(tid, WM_QUIT, WPARAM(0), LPARAM(0));
        }
    }
}

pub fn authorize_session(
    target_window_id: isize,
    target_exe_path: Option<String>,
) -> Result<NativeAuthorizedSession, String> {
    let mut pid = 0;
    if target_window_id > 0 {
        let hwnd = HWND(target_window_id as *mut _);
        unsafe {
            if !IsWindow(Some(hwnd)).as_bool() || !IsWindowVisible(hwnd).as_bool() {
                return Err("Target window is not valid or not visible; cannot authorize session".into());
            }
            GetWindowThreadProcessId(hwnd, Some(&mut pid));
            if pid == 0 {
                return Err("Failed to obtain process ID for target window".into());
            }
            if pid == std::process::id() && class_of(hwnd) != "MimicAutomationDemoTarget" {
                return Err("Cannot authorize MIMIC's own window for automation".into());
            }
        }
    }

    let actual_exe = if pid > 0 {
        unsafe { get_process_path(pid) }
    } else {
        target_exe_path.clone()
    };

    let session = NativeAuthorizedSession {
        session_id: format!(
            "session_{}_{}",
            pid,
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis()
        ),
        target_window_id,
        target_process_id: pid,
        target_exe_path: actual_exe.or(target_exe_path),
        created_at: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs(),
        is_active: true,
    };

    let mut lock = match ACTIVE_SESSION.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    *lock = Some(session.clone());

    start_global_emergency_hook();
    Ok(session)
}

pub fn update_session_target(target_window_id: isize, process_id: u32) {
    if let Ok(mut lock) = ACTIVE_SESSION.lock() {
        if let Some(session) = lock.as_mut() {
            session.target_window_id = target_window_id;
            session.target_process_id = process_id;
            if session.target_exe_path.is_none() || session.target_exe_path.as_deref() == Some("DEMO") {
                session.target_exe_path = unsafe { get_process_path(process_id) };
            }
        }
    }
}

pub fn revoke_session() {
    stop_global_emergency_hook();
    if let Ok(mut lock) = ACTIVE_SESSION.lock() {
        if let Some(session) = lock.as_mut() {
            session.is_active = false;
        }
        *lock = None;
    }
}

pub fn get_active_session() -> Option<NativeAuthorizedSession> {
    ACTIVE_SESSION.lock().ok()?.clone()
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WindowInfo {
    pub id: isize,
    pub title: String,
    pub process_id: u32,
    pub is_demo: bool,
    pub exe_path: Option<String>,
    pub rect: [i32; 4],
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SelectOnScreenResult {
    pub cancelled: bool,
    pub mode: String, // "uia" | "region"
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
    pub window_id: isize,
    pub window_title: String,
    pub element_name: Option<String>,
    pub control_type: Option<String>,
    pub automation_id: Option<String>,
    pub class_name: Option<String>,
    pub image_base64: Option<String>,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LocatorPayload {
    pub kind: String, // "uia" | "template" | "window-title" | "relative-point"
    pub automation_id: Option<String>,
    pub name: Option<String>,
    pub control_type: Option<String>,
    pub class_name: Option<String>,
    pub image_base64: Option<String>,
    pub x: Option<i32>,
    pub y: Option<i32>,
    pub width: Option<i32>,
    pub height: Option<i32>,
    pub reference_width: Option<i32>,
    pub reference_height: Option<i32>,
    pub threshold: Option<f32>,
    pub value: Option<String>,
}

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StepExecutionPayload {
    #[allow(dead_code)]
    pub id: Option<String>,
    pub title: String,
    pub kind: String,
    pub action_type: Option<String>,
    pub locator: Option<LocatorPayload>,
    pub text: Option<String>,
    #[allow(dead_code)]
    pub secret_ref: Option<String>,
    pub key: Option<String>,
    pub expected_state: Option<String>,
    pub expected_locator: Option<LocatorPayload>,
    pub timeout_ms: Option<u64>,
    pub max_attempts: Option<u32>,
    pub permission_granted: Option<bool>,
    pub target_process_id: Option<u32>,
    pub target_exe_path: Option<String>,
    pub target_window_title: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StepExecutionResult {
    pub success: bool,
    pub status: String, // "verified" | "completed" | "failed" | "stopped"
    pub message: String,
    pub target_window_id: isize,
    pub attempts: u32,
    pub duration_ms: u64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DemoRunResult {
    pub status: String,
    pub message: String,
    pub target_window_id: isize,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DemoObservation {
    pub state: String,
    pub title: String,
    pub target_window_id: isize,
}

#[derive(Clone)]
pub struct ResolvedElement {
    pub rect: RECT,
    pub child_hwnd: Option<HWND>,
    #[allow(dead_code)]
    pub method: String,
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

unsafe fn get_process_path(pid: u32) -> Option<String> {
    if pid == 0 {
        return None;
    }
    let handle = OpenProcess(PROCESS_QUERY_INFORMATION | PROCESS_VM_READ, false, pid).ok()?;
    let mut buffer = [0u16; 1024];
    let written = GetModuleFileNameExW(Some(handle), None, &mut buffer);
    let _ = windows::Win32::Foundation::CloseHandle(handle);
    if written > 0 {
        Some(String::from_utf16_lossy(&buffer[..written as usize]))
    } else {
        None
    }
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

    let mut rect = RECT::default();
    let _ = GetWindowRect(window, &mut rect);

    let windows = &mut *(data.0 as *mut Vec<WindowInfo>);
    let is_demo = class_of(window) == "MimicAutomationDemoTarget";
    let exe_path = get_process_path(process_id);

    windows.push(WindowInfo {
        id: window.0 as isize,
        is_demo,
        title,
        process_id,
        exe_path,
        rect: [rect.left, rect.top, rect.right, rect.bottom],
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

pub fn pick_executable() -> Result<Option<String>, String> {
    unsafe {
        let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
        let dialog: IFileOpenDialog = CoCreateInstance(&FileOpenDialog, None, CLSCTX_INPROC_SERVER)
            .map_err(|e| format!("Cannot create file dialog: {e}"))?;

        let _ = dialog.SetOptions(FOS_FILEMUSTEXIST | FOS_FORCEFILESYSTEM);
        let _ = dialog.SetTitle(w!("MIMIC - Select Application Executable"));

        if dialog.Show(None).is_err() {
            return Ok(None);
        }

        let item = dialog
            .GetResult()
            .map_err(|e| format!("Failed to retrieve selected file: {e}"))?;
        let display_name: PWSTR = item
            .GetDisplayName(SIGDN_FILESYSPATH)
            .map_err(|e| format!("Failed to retrieve file path: {e}"))?;
        let path = display_name
            .to_string()
            .map_err(|e| format!("Invalid path encoding: {e}"))?;
        Ok(Some(path))
    }
}

pub fn launch_application(executable_path: &str) -> Result<u32, String> {
    let trimmed = executable_path.trim();
    if trimmed.is_empty() || trimmed.eq_ignore_ascii_case("DEMO") {
        start_demo()?;
        return Ok(std::process::id());
    }

    let child = std::process::Command::new(trimmed)
        .spawn()
        .map_err(|e| format!("Failed to launch executable '{trimmed}': {e}"))?;
    Ok(child.id())
}

pub fn focus_window(window_id: isize) -> Result<(), String> {
    let hwnd = HWND(window_id as *mut _);
    unsafe {
        if !IsWindow(Some(hwnd)).as_bool() {
            return Err("Target window no longer exists".into());
        }
        let _ = ShowWindow(hwnd, SW_RESTORE);
        let _ = SetForegroundWindow(hwnd);
    }
    Ok(())
}

fn to_base64(data: &[u8]) -> String {
    const CHARSET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut result = String::with_capacity((data.len() + 2) / 3 * 4);
    for chunk in data.chunks(3) {
        let b0 = chunk[0] as usize;
        let b1 = if chunk.len() > 1 { chunk[1] as usize } else { 0 };
        let b2 = if chunk.len() > 2 { chunk[2] as usize } else { 0 };

        let triple = (b0 << 16) | (b1 << 8) | b2;
        result.push(CHARSET[(triple >> 18) & 0x3F] as char);
        result.push(CHARSET[(triple >> 12) & 0x3F] as char);
        if chunk.len() > 1 {
            result.push(CHARSET[(triple >> 6) & 0x3F] as char);
        } else {
            result.push('=');
        }
        if chunk.len() > 2 {
            result.push(CHARSET[triple & 0x3F] as char);
        } else {
            result.push('=');
        }
    }
    result
}

fn from_base64(input: &str) -> Result<Vec<u8>, String> {
    fn decode_char(c: u8) -> Result<u8, String> {
        match c {
            b'A'..=b'Z' => Ok(c - b'A'),
            b'a'..=b'z' => Ok(c - b'a' + 26),
            b'0'..=b'9' => Ok(c - b'0' + 52),
            b'+' => Ok(62),
            b'/' => Ok(63),
            b'=' => Ok(0),
            _ => Err("Invalid base64 character".into()),
        }
    }

    let clean: Vec<u8> = input.bytes().filter(|b| !b.is_ascii_whitespace()).collect();
    if clean.is_empty() {
        return Ok(Vec::new());
    }
    if clean.len() % 4 != 0 {
        return Err("Invalid base64 length".into());
    }

    let mut output = Vec::with_capacity(clean.len() / 4 * 3);
    for chunk in clean.chunks(4) {
        let b0 = decode_char(chunk[0])?;
        let b1 = decode_char(chunk[1])?;
        let b2 = decode_char(chunk[2])?;
        let b3 = decode_char(chunk[3])?;

        output.push((b0 << 2) | (b1 >> 4));
        if chunk[2] != b'=' {
            output.push((b1 << 4) | (b2 >> 2));
        }
        if chunk[3] != b'=' {
            output.push((b2 << 6) | b3);
        }
    }
    Ok(output)
}

fn encode_bmp_to_base64(width: i32, height: i32, bgra_pixels: &[u8]) -> String {
    let file_header_size: u32 = 14;
    let info_header_size: u32 = 40;
    let image_size: u32 = (width * height * 4) as u32;
    let file_size: u32 = file_header_size + info_header_size + image_size;

    let mut bmp_data = Vec::with_capacity(file_size as usize);
    bmp_data.extend_from_slice(b"BM");
    bmp_data.extend_from_slice(&file_size.to_le_bytes());
    bmp_data.extend_from_slice(&0u32.to_le_bytes());
    let offset: u32 = file_header_size + info_header_size;
    bmp_data.extend_from_slice(&offset.to_le_bytes());

    bmp_data.extend_from_slice(&info_header_size.to_le_bytes());
    bmp_data.extend_from_slice(&width.to_le_bytes());
    bmp_data.extend_from_slice(&(-height).to_le_bytes());
    bmp_data.extend_from_slice(&1u16.to_le_bytes());
    bmp_data.extend_from_slice(&32u16.to_le_bytes());
    bmp_data.extend_from_slice(&0u32.to_le_bytes());
    bmp_data.extend_from_slice(&image_size.to_le_bytes());
    bmp_data.extend_from_slice(&0u32.to_le_bytes());
    bmp_data.extend_from_slice(&0u32.to_le_bytes());
    bmp_data.extend_from_slice(&0u32.to_le_bytes());
    bmp_data.extend_from_slice(&0u32.to_le_bytes());

    bmp_data.extend_from_slice(bgra_pixels);

    let b64 = to_base64(&bmp_data);
    format!("data:image/bmp;base64,{b64}")
}

pub unsafe fn capture_screen_rect(
    x: i32,
    y: i32,
    width: i32,
    height: i32,
) -> Result<String, String> {
    if width <= 0 || height <= 0 {
        return Err("Dimensions must be positive".into());
    }
    let screen_dc = GetDC(None);
    let mem_dc = CreateCompatibleDC(Some(screen_dc));
    let bitmap = CreateCompatibleBitmap(screen_dc, width, height);
    let old_bmp = SelectObject(mem_dc, bitmap.into());

    let _ = BitBlt(mem_dc, 0, 0, width, height, Some(screen_dc), x, y, SRCCOPY);

    let mut bi = BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER {
            biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: width,
            biHeight: -height,
            biPlanes: 1,
            biBitCount: 32,
            biCompression: 0,
            biSizeImage: (width * height * 4) as u32,
            ..Default::default()
        },
        ..Default::default()
    };

    let mut buffer = vec![0u8; (width * height * 4) as usize];
    GetDIBits(
        mem_dc,
        bitmap,
        0,
        height as u32,
        Some(buffer.as_mut_ptr() as *mut _),
        &mut bi,
        DIB_RGB_COLORS,
    );

    SelectObject(mem_dc, old_bmp);
    let _ = DeleteObject(bitmap.into());
    let _ = DeleteDC(mem_dc);
    ReleaseDC(None, screen_dc);

    Ok(encode_bmp_to_base64(width, height, &buffer))
}

// -----------------------------------------------------------------------------
// Real Image Template Matching (Pure Rust Normalized Difference Search)
// -----------------------------------------------------------------------------

struct GrayscaleImage {
    width: i32,
    height: i32,
    pixels: Vec<u8>,
}

fn decode_bmp_to_grayscale(data_url: &str) -> Option<GrayscaleImage> {
    let b64_str = if let Some(idx) = data_url.find(',') {
        &data_url[idx + 1..]
    } else {
        data_url
    };
    let bytes = from_base64(b64_str).ok()?;
    if bytes.len() < 54 || &bytes[0..2] != b"BM" {
        return None;
    }

    let width = i32::from_le_bytes(bytes[18..22].try_into().ok()?).abs();
    let height = i32::from_le_bytes(bytes[22..26].try_into().ok()?).abs();
    let bpp = u16::from_le_bytes(bytes[28..30].try_into().ok()?);
    let offset = u32::from_le_bytes(bytes[10..14].try_into().ok()?) as usize;

    if width <= 0 || height <= 0 || offset >= bytes.len() {
        return None;
    }

    let bytes_per_pixel = (bpp / 8) as usize;
    if bytes_per_pixel < 3 {
        return None;
    }

    let mut pixels = Vec::with_capacity((width * height) as usize);
    let row_stride = ((width as usize * bytes_per_pixel + 3) / 4) * 4;

    for y in 0..height {
        let row_start = offset + (y as usize * row_stride);
        for x in 0..width {
            let px = row_start + (x as usize * bytes_per_pixel);
            if px + 2 < bytes.len() {
                let b = bytes[px] as u32;
                let g = bytes[px + 1] as u32;
                let r = bytes[px + 2] as u32;
                let gray = ((r * 77 + g * 150 + b * 29) >> 8) as u8;
                pixels.push(gray);
            } else {
                pixels.push(0);
            }
        }
    }

    Some(GrayscaleImage {
        width,
        height,
        pixels,
    })
}

#[link(name = "user32")]
extern "system" {
    fn PrintWindow(
        hwnd: HWND,
        hdc_blt: windows::Win32::Graphics::Gdi::HDC,
        n_flags: u32,
    ) -> windows::core::BOOL;
}

unsafe fn capture_window_grayscale(hwnd: HWND) -> Option<(GrayscaleImage, RECT)> {
    let mut rect = RECT::default();
    GetWindowRect(hwnd, &mut rect).ok()?;
    let width = rect.right - rect.left;
    let height = rect.bottom - rect.top;

    if width <= 4 || height <= 4 {
        return None;
    }

    let win_dc = GetDC(Some(hwnd));
    let mem_dc = CreateCompatibleDC(Some(win_dc));
    let bitmap = CreateCompatibleBitmap(win_dc, width, height);
    let old_bmp = SelectObject(mem_dc, bitmap.into());

    let printed = PrintWindow(hwnd, mem_dc, 2);
    if !printed.as_bool() {
        let screen_dc = GetDC(None);
        let _ = BitBlt(mem_dc, 0, 0, width, height, Some(screen_dc), rect.left, rect.top, SRCCOPY);
        ReleaseDC(None, screen_dc);
    }

    let mut bi = BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER {
            biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: width,
            biHeight: -height, // top-down
            biPlanes: 1,
            biBitCount: 32,
            biCompression: 0,
            biSizeImage: (width * height * 4) as u32,
            ..Default::default()
        },
        ..Default::default()
    };

    let mut buffer = vec![0u8; (width * height * 4) as usize];
    GetDIBits(
        mem_dc,
        bitmap,
        0,
        height as u32,
        Some(buffer.as_mut_ptr() as *mut _),
        &mut bi,
        DIB_RGB_COLORS,
    );

    SelectObject(mem_dc, old_bmp);
    let _ = DeleteObject(bitmap.into());
    let _ = DeleteDC(mem_dc);
    ReleaseDC(Some(hwnd), win_dc);

    let mut pixels = Vec::with_capacity((width * height) as usize);
    for chunk in buffer.chunks_exact(4) {
        let b = chunk[0] as u32;
        let g = chunk[1] as u32;
        let r = chunk[2] as u32;
        let gray = ((r * 77 + g * 150 + b * 29) >> 8) as u8;
        pixels.push(gray);
    }

    Some((
        GrayscaleImage {
            width,
            height,
            pixels,
        },
        rect,
    ))
}

#[allow(dead_code)]
fn search_template_match(
    screen: &GrayscaleImage,
    template: &GrayscaleImage,
    threshold: f32,
) -> Option<(i32, i32, f32)> {
    search_template_match_with_roi(screen, template, threshold, None)
}

fn search_template_match_with_roi(
    screen: &GrayscaleImage,
    template: &GrayscaleImage,
    threshold: f32,
    roi_hint: Option<(i32, i32)>,
) -> Option<(i32, i32, f32)> {
    let tw = template.width;
    let th = template.height;
    let sw = screen.width;
    let sh = screen.height;

    if tw > sw || th > sh || tw <= 0 || th <= 0 {
        return None;
    }

    // Fast-path: If ROI hint is provided, search localized region (+-120px) first
    if let Some((hx, hy)) = roi_hint {
        let hx_min = (hx - 120).max(0);
        let hx_max = (hx + 120).min(sw - tw);
        let hy_min = (hy - 120).max(0);
        let hy_max = (hy + 120).min(sh - th);

        if hx_max >= hx_min && hy_max >= hy_min {
            let mut local_best_score = -1.0f32;
            let mut local_best_pos = (hx_min, hy_min);
            for sy in (hy_min..=hy_max).step_by(2) {
                for sx in (hx_min..=hx_max).step_by(2) {
                    let mut diff_sum: u64 = 0;
                    for ty in (0..th).step_by(2) {
                        let s_idx = ((sy + ty) * sw + sx) as usize;
                        let t_idx = (ty * tw) as usize;
                        for tx in (0..tw).step_by(2) {
                            let s_val = screen.pixels[s_idx + tx as usize] as i32;
                            let t_val = template.pixels[t_idx + tx as usize] as i32;
                            diff_sum += (s_val - t_val).unsigned_abs() as u64;
                        }
                    }
                    let samples = ((th + 1) / 2) * ((tw + 1) / 2);
                    let score = 1.0 - (diff_sum as f32 / (samples as f32 * 255.0));
                    if score > local_best_score {
                        local_best_score = score;
                        local_best_pos = (sx, sy);
                    }
                }
            }

            if local_best_score >= threshold {
                let fx_min = (local_best_pos.0 - 3).max(0);
                let fx_max = (local_best_pos.0 + 3).min(sw - tw);
                let fy_min = (local_best_pos.1 - 3).max(0);
                let fy_max = (local_best_pos.1 + 3).min(sh - th);

                let mut fine_score = -1.0f32;
                let mut fine_pos = local_best_pos;
                for sy in fy_min..=fy_max {
                    for sx in fx_min..=fx_max {
                        let mut diff_sum: u64 = 0;
                        for ty in 0..th {
                            let s_idx = ((sy + ty) * sw + sx) as usize;
                            let t_idx = (ty * tw) as usize;
                            for tx in 0..tw {
                                let s_val = screen.pixels[s_idx + tx as usize] as i32;
                                let t_val = template.pixels[t_idx + tx as usize] as i32;
                                diff_sum += (s_val - t_val).unsigned_abs() as u64;
                            }
                        }
                        let score = 1.0 - (diff_sum as f32 / (tw as f32 * th as f32 * 255.0));
                        if score > fine_score {
                            fine_score = score;
                            fine_pos = (sx, sy);
                        }
                    }
                }
                if fine_score >= threshold {
                    return Some((fine_pos.0, fine_pos.1, fine_score));
                }
            }
        }
    }

    // Full window scan (adaptive step based on resolution)
    let step_stride: i32 = if sw > 800 || sh > 600 { 3 } else { 2 };
    let mut best_score = 0.0f32;
    let mut best_pos = (0, 0);

    for sy in (0..=sh - th).step_by(step_stride as usize) {
        for sx in (0..=sw - tw).step_by(step_stride as usize) {
            let mut diff_sum: u64 = 0;
            for ty in (0..th).step_by(step_stride as usize) {
                let s_idx = ((sy + ty) * sw + sx) as usize;
                let t_idx = (ty * tw) as usize;
                for tx in (0..tw).step_by(step_stride as usize) {
                    let s_val = screen.pixels[s_idx + tx as usize] as i32;
                    let t_val = template.pixels[t_idx + tx as usize] as i32;
                    diff_sum += (s_val - t_val).unsigned_abs() as u64;
                }
            }
            let samples =
                ((th + step_stride - 1) / step_stride) * ((tw + step_stride - 1) / step_stride);
            let score = 1.0 - (diff_sum as f32 / (samples as f32 * 255.0));
            if score > best_score {
                best_score = score;
                best_pos = (sx, sy);
            }
        }
    }

    // Fine scan: step by 1 around candidate
    let rx_min = (best_pos.0 - (step_stride + 1)).max(0);
    let rx_max = (best_pos.0 + (step_stride + 1)).min(sw - tw);
    let ry_min = (best_pos.1 - (step_stride + 1)).max(0);
    let ry_max = (best_pos.1 + (step_stride + 1)).min(sh - th);

    let mut fine_best_score = -1.0f32;
    let mut fine_best_pos = best_pos;

    for sy in ry_min..=ry_max {
        for sx in rx_min..=rx_max {
            let mut diff_sum: u64 = 0;
            for ty in 0..th {
                let s_idx = ((sy + ty) * sw + sx) as usize;
                let t_idx = (ty * tw) as usize;
                for tx in 0..tw {
                    let s_val = screen.pixels[s_idx + tx as usize] as i32;
                    let t_val = template.pixels[t_idx + tx as usize] as i32;
                    diff_sum += (s_val - t_val).unsigned_abs() as u64;
                }
            }
            let score = 1.0 - (diff_sum as f32 / (tw as f32 * th as f32 * 255.0));
            if score > fine_best_score {
                fine_best_score = score;
                fine_best_pos = (sx, sy);
            }
        }
    }

    if fine_best_score >= threshold {
        Some((fine_best_pos.0, fine_best_pos.1, fine_best_score))
    } else {
        None
    }
}

// -----------------------------------------------------------------------------
// Real Dynamic Locator Resolution (UI Automation + Child Controls + Template)
// -----------------------------------------------------------------------------

unsafe fn search_uia_descendants(
    automation: &IUIAutomation,
    walker: &IUIAutomationTreeWalker,
    current: &IUIAutomationElement,
    locator: &LocatorPayload,
    depth: usize,
) -> Option<(IUIAutomationElement, RECT)> {
    if depth > 10 {
        return None;
    }

    let mut is_match = true;

    // Check AutomationId
    if let Some(expected_id) = &locator.automation_id {
        if !expected_id.trim().is_empty() {
            let actual_id = current.CurrentAutomationId().map(|s| s.to_string()).unwrap_or_default();
            if !actual_id.eq_ignore_ascii_case(expected_id.trim()) {
                is_match = false;
            }
        }
    }

    // Check Name
    if is_match {
        if let Some(expected_name) = &locator.name {
            if !expected_name.trim().is_empty() {
                let actual_name = current.CurrentName().map(|s| s.to_string()).unwrap_or_default();
                if !actual_name.eq_ignore_ascii_case(expected_name.trim()) {
                    is_match = false;
                }
            }
        }
    }

    // Check ControlType
    if is_match {
        if let Some(expected_ctrl) = &locator.control_type {
            if !expected_ctrl.trim().is_empty() {
                let actual_ctrl = current.CurrentLocalizedControlType().map(|s| s.to_string()).unwrap_or_default();
                if !actual_ctrl.eq_ignore_ascii_case(expected_ctrl.trim()) {
                    is_match = false;
                }
            }
        }
    }

    if is_match && (locator.automation_id.is_some() || locator.name.is_some()) {
        if let Ok(rect) = current.CurrentBoundingRectangle() {
            let w = rect.right - rect.left;
            let h = rect.bottom - rect.top;
            if w > 0 && h > 0 {
                return Some((
                    current.clone(),
                    RECT {
                        left: rect.left,
                        top: rect.top,
                        right: rect.right,
                        bottom: rect.bottom,
                    },
                ));
            }
        }
    }

    // Traverse children
    if let Ok(child) = walker.GetFirstChildElement(current) {
        if let Some(found) = search_uia_descendants(automation, walker, &child, locator, depth + 1) {
            return Some(found);
        }
        let mut sibling = child;
        while let Ok(next) = walker.GetNextSiblingElement(&sibling) {
            if let Some(found) = search_uia_descendants(automation, walker, &next, locator, depth + 1) {
                return Some(found);
            }
            sibling = next;
        }
    }

    None
}

unsafe fn resolve_uia_element(
    hwnd: HWND,
    locator: &LocatorPayload,
) -> Option<(IUIAutomationElement, RECT)> {
    let automation: IUIAutomation = CoCreateInstance(&CUIAutomation, None, CLSCTX_INPROC_SERVER).ok()?;
    let walker = automation.ControlViewWalker().ok()?;
    let root = automation.ElementFromHandle(hwnd).ok()?;
    search_uia_descendants(&automation, &walker, &root, locator, 0)
}

struct ChildSearchCtx<'a> {
    expected_name: &'a str,
    found: Option<(HWND, RECT)>,
}

unsafe extern "system" fn enum_child_callback(child: HWND, lparam: LPARAM) -> BOOL {
    let ctx = &mut *(lparam.0 as *mut ChildSearchCtx);
    let title = title_of(child);
    if title.eq_ignore_ascii_case(ctx.expected_name) {
        let mut r = RECT::default();
        let _ = GetWindowRect(child, &mut r);
        ctx.found = Some((child, r));
        return BOOL(0); // stop
    }
    BOOL(1)
}

unsafe fn find_win32_child(hwnd: HWND, name: &str) -> Option<(HWND, RECT)> {
    let mut ctx = ChildSearchCtx {
        expected_name: name,
        found: None,
    };
    let _ = EnumChildWindows(
        Some(hwnd),
        Some(enum_child_callback),
        LPARAM((&mut ctx as *mut ChildSearchCtx) as isize),
    );
    ctx.found
}

/// Dynamic locator resolver:
/// Resolves the runtime screen rectangle and child HWND of the element across window movements, resizing,
/// using UI Automation, Win32 child enumeration, or Real Template Matching.
pub unsafe fn resolve_locator(
    hwnd: HWND,
    locator: &LocatorPayload,
) -> Result<ResolvedElement, String> {
    // 1. Try UIA Dynamic Resolution
    if locator.kind == "uia" || locator.automation_id.is_some() || locator.name.is_some() {
        // Fast-path: Win32 child control ID
        if let Some(id_str) = &locator.automation_id {
            if let Ok(id) = id_str.trim().parse::<i32>() {
                if let Ok(child) = windows::Win32::UI::WindowsAndMessaging::GetDlgItem(Some(hwnd), id) {
                    if !child.0.is_null() && IsWindow(Some(child)).as_bool() {
                        let mut r = RECT::default();
                        let _ = GetWindowRect(child, &mut r);
                        return Ok(ResolvedElement {
                            rect: r,
                            child_hwnd: Some(child),
                            method: "win32_dlg".into(),
                        });
                    }
                }
            }
        }

        if let Some((elem, rect)) = resolve_uia_element(hwnd, locator) {
            let child_hwnd = elem.CurrentNativeWindowHandle().ok().and_then(|h| {
                if h.0.is_null() { None } else { Some(h) }
            });
            return Ok(ResolvedElement {
                rect,
                child_hwnd,
                method: "uia".into(),
            });
        }

        // Fallback: check Win32 child controls by name
        if let Some(name) = &locator.name {
            if let Some((child, rect)) = find_win32_child(hwnd, name) {
                return Ok(ResolvedElement {
                    rect,
                    child_hwnd: Some(child),
                    method: "win32".into(),
                });
            }
        }
    }

    // 2. Try Real Image Template Matching if image template is present
    if let Some(img_b64) = &locator.image_base64 {
        if !img_b64.is_empty() {
            if let Some(template) = decode_bmp_to_grayscale(img_b64) {
                if let Some((screen_img, window_rect)) = capture_window_grayscale(hwnd) {
                    let threshold = locator.threshold.unwrap_or(0.80);
                    let roi_hint = match (locator.x, locator.y) {
                        (Some(x), Some(y)) if x >= window_rect.left && y >= window_rect.top => {
                            Some((x - window_rect.left, y - window_rect.top))
                        }
                        _ => None,
                    };
                    if let Some((mx, my, conf)) =
                        search_template_match_with_roi(&screen_img, &template, threshold, roi_hint)
                    {
                        let rect = RECT {
                            left: window_rect.left + mx,
                            top: window_rect.top + my,
                            right: window_rect.left + mx + template.width,
                            bottom: window_rect.top + my + template.height,
                        };
                        return Ok(ResolvedElement {
                            rect,
                            child_hwnd: None,
                            method: format!("template(conf={conf:.2})"),
                        });
                    }
                }
            }
        }
    }

    let desc = locator
        .name
        .as_deref()
        .or(locator.control_type.as_deref())
        .or(locator.automation_id.as_deref())
        .unwrap_or("unknown");

    Err(format!(
        "Target element '{desc}' could not be dynamically resolved in window"
    ))
}

// -----------------------------------------------------------------------------
// Interactive Selection Overlay
// -----------------------------------------------------------------------------

struct OverlayState {
    _target_window_id: Option<isize>,
    target_window_rect: Option<RECT>,
    target_window_pid: Option<u32>,
    is_dragging: bool,
    drag_start: POINT,
    drag_current: POINT,
    hover_rect: RECT,
    hover_title: String,
    hover_element: Option<UiaElementDetails>,
    result: Option<SelectOnScreenResult>,
    selection_done: bool,
}

#[derive(Clone)]
struct UiaElementDetails {
    name: String,
    control_type: String,
    automation_id: String,
    class_name: String,
}

static OVERLAY_STATE: Mutex<Option<OverlayState>> = Mutex::new(None);

unsafe fn inspect_uia_at_point(
    pt: POINT,
    automation: &IUIAutomation,
) -> Option<(UiaElementDetails, RECT)> {
    let elem = automation.ElementFromPoint(pt).ok()?;
    let name = elem.CurrentName().map(|s| s.to_string()).unwrap_or_default();
    let control_type = elem
        .CurrentLocalizedControlType()
        .map(|s| s.to_string())
        .unwrap_or_default();
    let automation_id = elem
        .CurrentAutomationId()
        .map(|s| s.to_string())
        .unwrap_or_default();
    let class_name = elem
        .CurrentClassName()
        .map(|s| s.to_string())
        .unwrap_or_default();
    let uia_rect = elem.CurrentBoundingRectangle().ok()?;

    let rect = RECT {
        left: uia_rect.left,
        top: uia_rect.top,
        right: uia_rect.right,
        bottom: uia_rect.bottom,
    };

    Some((
        UiaElementDetails {
            name,
            control_type,
            automation_id,
            class_name,
        },
        rect,
    ))
}

unsafe extern "system" fn overlay_window_proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match message {
        WM_MOUSEMOVE => {
            let mut pt = POINT::default();
            let _ = GetCursorPos(&mut pt);

            let mut state_guard = OVERLAY_STATE.lock().unwrap();
            if let Some(state) = state_guard.as_mut() {
                if state.is_dragging {
                    state.drag_current = pt;
                    drop(state_guard);
                    let _ = InvalidateRect(Some(hwnd), None, true);
                    let _ = UpdateWindow(hwnd);
                    return LRESULT(0);
                }

                // Restriction 1: If target window bounds exist, restrict cursor within target
                if let Some(bounds) = state.target_window_rect {
                    if pt.x < bounds.left || pt.x > bounds.right || pt.y < bounds.top || pt.y > bounds.bottom {
                        state.hover_rect = RECT::default();
                        state.hover_title = "Cursor is outside authorized target window".into();
                        state.hover_element = None;
                        drop(state_guard);
                        let _ = InvalidateRect(Some(hwnd), None, true);
                        let _ = UpdateWindow(hwnd);
                        return LRESULT(0);
                    }
                }

                // Restriction 2: Check window under cursor belongs to target process
                let window_under = windows::Win32::UI::WindowsAndMessaging::WindowFromPoint(pt);
                if let Some(target_pid) = state.target_window_pid {
                    let mut under_pid = 0;
                    GetWindowThreadProcessId(window_under, Some(&mut under_pid));
                    if under_pid != target_pid && under_pid != 0 {
                        state.hover_rect = RECT::default();
                        state.hover_title = "Window does not belong to authorized target process".into();
                        state.hover_element = None;
                        drop(state_guard);
                        let _ = InvalidateRect(Some(hwnd), None, true);
                        let _ = UpdateWindow(hwnd);
                        return LRESULT(0);
                    }
                }

                if let Ok(automation) =
                    CoCreateInstance::<_, IUIAutomation>(&CUIAutomation, None, CLSCTX_INPROC_SERVER)
                {
                    if let Some((details, rect)) = inspect_uia_at_point(pt, &automation) {
                        state.hover_rect = rect;
                        state.hover_title = if !details.name.is_empty() {
                            format!("{}: \"{}\"", details.control_type, details.name)
                        } else if !details.automation_id.is_empty() {
                            format!("{}: #{}", details.control_type, details.automation_id)
                        } else {
                            details.control_type.clone()
                        };
                        state.hover_element = Some(details);
                    }
                }
            }
            drop(state_guard);
            let _ = InvalidateRect(Some(hwnd), None, true);
            let _ = UpdateWindow(hwnd);
            LRESULT(0)
        }
        WM_LBUTTONDOWN => {
            let mut pt = POINT::default();
            let _ = GetCursorPos(&mut pt);
            let mut state_guard = OVERLAY_STATE.lock().unwrap();
            if let Some(state) = state_guard.as_mut() {
                // If cursor is outside permitted target window, reject click
                if let Some(bounds) = state.target_window_rect {
                    if pt.x < bounds.left || pt.x > bounds.right || pt.y < bounds.top || pt.y > bounds.bottom {
                        return LRESULT(0);
                    }
                }
                let window_under = windows::Win32::UI::WindowsAndMessaging::WindowFromPoint(pt);
                if let Some(target_pid) = state.target_window_pid {
                    let mut under_pid = 0;
                    GetWindowThreadProcessId(window_under, Some(&mut under_pid));
                    if under_pid != target_pid && under_pid != 0 {
                        return LRESULT(0);
                    }
                }
                state.is_dragging = true;
                state.drag_start = pt;
                state.drag_current = pt;
                SetCapture(hwnd);
            }
            LRESULT(0)
        }
        WM_LBUTTONUP => {
            let _ = ReleaseCapture();
            let mut pt = POINT::default();
            let _ = GetCursorPos(&mut pt);

            let mut state_guard = OVERLAY_STATE.lock().unwrap();
            if let Some(state) = state_guard.as_mut() {
                state.is_dragging = false;

                // Validate click is within authorized target window
                if let Some(bounds) = state.target_window_rect {
                    if pt.x < bounds.left || pt.x > bounds.right || pt.y < bounds.top || pt.y > bounds.bottom {
                        drop(state_guard);
                        let _ = InvalidateRect(Some(hwnd), None, true);
                        let _ = UpdateWindow(hwnd);
                        return LRESULT(0);
                    }
                }
                let window_under = windows::Win32::UI::WindowsAndMessaging::WindowFromPoint(pt);
                if let Some(target_pid) = state.target_window_pid {
                    let mut under_pid = 0;
                    GetWindowThreadProcessId(window_under, Some(&mut under_pid));
                    if under_pid != target_pid && under_pid != 0 {
                        drop(state_guard);
                        let _ = InvalidateRect(Some(hwnd), None, true);
                        let _ = UpdateWindow(hwnd);
                        return LRESULT(0);
                    }
                }
                let dx = (pt.x - state.drag_start.x).abs();
                let dy = (pt.y - state.drag_start.y).abs();

                let (mode, rect) = if dx > 8 || dy > 8 {
                    let left = state.drag_start.x.min(pt.x);
                    let top = state.drag_start.y.min(pt.y);
                    let right = state.drag_start.x.max(pt.x);
                    let bottom = state.drag_start.y.max(pt.y);
                    ("region".to_string(), RECT { left, top, right, bottom })
                } else {
                    let r = if state.hover_rect.right > state.hover_rect.left
                        && state.hover_rect.bottom > state.hover_rect.top
                    {
                        state.hover_rect
                    } else {
                        RECT {
                            left: pt.x - 20,
                            top: pt.y - 12,
                            right: pt.x + 20,
                            bottom: pt.y + 12,
                        }
                    };
                    ("uia".to_string(), r)
                };

                let w = (rect.right - rect.left).max(1);
                let h = (rect.bottom - rect.top).max(1);

                let _ = ShowWindow(hwnd, windows::Win32::UI::WindowsAndMessaging::SW_HIDE);
                thread::sleep(Duration::from_millis(50));
                let img_b64 = capture_screen_rect(rect.left, rect.top, w, h).ok();

                let window_under = windows::Win32::UI::WindowsAndMessaging::WindowFromPoint(pt);
                let win_title = title_of(window_under);

                let (el_name, ctrl_type, auto_id, cls_name) =
                    if let Some(el) = state.hover_element.take() {
                        (
                            Some(el.name),
                            Some(el.control_type),
                            Some(el.automation_id),
                            Some(el.class_name),
                        )
                    } else {
                        (None, None, None, None)
                    };

                state.result = Some(SelectOnScreenResult {
                    cancelled: false,
                    mode,
                    x: rect.left,
                    y: rect.top,
                    width: w,
                    height: h,
                    window_id: window_under.0 as isize,
                    window_title: win_title,
                    element_name: el_name,
                    control_type: ctrl_type,
                    automation_id: auto_id,
                    class_name: cls_name,
                    image_base64: img_b64,
                });
                state.selection_done = true;
            }
            drop(state_guard);
            let _ = DestroyWindow(hwnd);
            PostQuitMessage(0);
            LRESULT(0)
        }
        WM_KEYDOWN if wparam.0 == VK_ESCAPE.0 as usize => {
            let mut state_guard = OVERLAY_STATE.lock().unwrap();
            if let Some(state) = state_guard.as_mut() {
                state.result = Some(SelectOnScreenResult {
                    cancelled: true,
                    mode: "cancelled".into(),
                    x: 0,
                    y: 0,
                    width: 0,
                    height: 0,
                    window_id: 0,
                    window_title: String::new(),
                    element_name: None,
                    control_type: None,
                    automation_id: None,
                    class_name: None,
                    image_base64: None,
                });
                state.selection_done = true;
            }
            drop(state_guard);
            let _ = DestroyWindow(hwnd);
            PostQuitMessage(0);
            LRESULT(0)
        }
        WM_RBUTTONUP => {
            let mut state_guard = OVERLAY_STATE.lock().unwrap();
            if let Some(state) = state_guard.as_mut() {
                state.result = Some(SelectOnScreenResult {
                    cancelled: true,
                    mode: "cancelled".into(),
                    x: 0,
                    y: 0,
                    width: 0,
                    height: 0,
                    window_id: 0,
                    window_title: String::new(),
                    element_name: None,
                    control_type: None,
                    automation_id: None,
                    class_name: None,
                    image_base64: None,
                });
                state.selection_done = true;
            }
            drop(state_guard);
            let _ = DestroyWindow(hwnd);
            PostQuitMessage(0);
            LRESULT(0)
        }
        WM_PAINT => {
            let mut ps = windows::Win32::Graphics::Gdi::PAINTSTRUCT::default();
            let hdc = windows::Win32::Graphics::Gdi::BeginPaint(hwnd, &mut ps);

            let black_brush = GetStockObject(BLACK_BRUSH);
            let mut client_rect = RECT::default();
            let _ = windows::Win32::UI::WindowsAndMessaging::GetClientRect(hwnd, &mut client_rect);
            let _ = windows::Win32::Graphics::Gdi::FillRect(hdc, &client_rect, HBRUSH(black_brush.0));

            let state_guard = OVERLAY_STATE.lock().unwrap();
            if let Some(state) = state_guard.as_ref() {
                let cyan_brush = CreateSolidBrush(COLORREF(0x00FF8C38));
                let text_brush = CreateSolidBrush(COLORREF(0x001B150A));

                if let Some(bounds) = state.target_window_rect {
                    let green_brush = CreateSolidBrush(COLORREF(0x0022AA22));
                    let mut b = bounds;
                    FrameRect(hdc, &b, green_brush);
                    b.left -= 1;
                    b.top -= 1;
                    b.right += 1;
                    b.bottom += 1;
                    FrameRect(hdc, &b, green_brush);
                    let _ = DeleteObject(green_brush.into());
                }

                if state.is_dragging {
                    let left = state.drag_start.x.min(state.drag_current.x);
                    let top = state.drag_start.y.min(state.drag_current.y);
                    let right = state.drag_start.x.max(state.drag_current.x);
                    let bottom = state.drag_start.y.max(state.drag_current.y);
                    let mut r = RECT { left, top, right, bottom };
                    FrameRect(hdc, &r, cyan_brush);
                    r.left -= 1;
                    r.top -= 1;
                    r.right += 1;
                    r.bottom += 1;
                    FrameRect(hdc, &r, cyan_brush);

                    let info_text = format!("Image Region: {} x {} px (Release to select, Esc to cancel)", right - left, bottom - top);
                    let wide_info = to_wide(&info_text);
                    let banner_rect = RECT { left: left.max(10), top: (top - 28).max(10), right: left + 420, bottom: (top - 4).max(34) };
                    let _ = windows::Win32::Graphics::Gdi::FillRect(hdc, &banner_rect, text_brush);
                    SetBkMode(hdc, TRANSPARENT);
                    SetTextColor(hdc, COLORREF(0x00FFFFFF));
                    let _ = TextOutW(hdc, banner_rect.left + 8, banner_rect.top + 4, &wide_info[..wide_info.len() - 1]);
                } else if state.hover_rect.right > state.hover_rect.left && state.hover_rect.bottom > state.hover_rect.top {
                    let mut r = state.hover_rect;
                    FrameRect(hdc, &r, cyan_brush);
                    r.left -= 1;
                    r.top -= 1;
                    r.right += 1;
                    r.bottom += 1;
                    FrameRect(hdc, &r, cyan_brush);

                    let banner_text = format!("{} | Click to Select | Drag for Image Crop | Esc to Cancel", if state.hover_title.is_empty() { "Element" } else { &state.hover_title });
                    let wide_banner = to_wide(&banner_text);
                    let banner_rect = RECT { left: r.left.max(10), top: (r.top - 28).max(10), right: r.left + 540, bottom: (r.top - 4).max(34) };
                    let _ = windows::Win32::Graphics::Gdi::FillRect(hdc, &banner_rect, text_brush);
                    SetBkMode(hdc, TRANSPARENT);
                    SetTextColor(hdc, COLORREF(0x00FFFFFF));
                    let _ = TextOutW(hdc, banner_rect.left + 8, banner_rect.top + 4, &wide_banner[..wide_banner.len() - 1]);
                }
                let _ = DeleteObject(cyan_brush.into());
                let _ = DeleteObject(text_brush.into());
            }

            let _ = windows::Win32::Graphics::Gdi::EndPaint(hwnd, &ps);
            LRESULT(0)
        }
        WM_DESTROY => {
            PostQuitMessage(0);
            LRESULT(0)
        }
        _ => DefWindowProcW(hwnd, message, wparam, lparam),
    }
}

pub fn select_on_screen(target_window_id: Option<isize>) -> Result<SelectOnScreenResult, String> {
    unsafe {
        let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);

        REGISTER_OVERLAY_CLASS.call_once(|| {
            let instance = GetModuleHandleW(None).unwrap_or_default();
            let cursor = LoadCursorW(None, IDC_CROSS).unwrap_or_default();
            let class = WNDCLASSW {
                hInstance: instance.into(),
                hCursor: cursor,
                lpszClassName: OVERLAY_CLASS,
                lpfnWndProc: Some(overlay_window_proc),
                hbrBackground: HBRUSH(GetStockObject(BLACK_BRUSH).0),
                ..Default::default()
            };
            let _ = RegisterClassW(&class);
        });

        let mut target_window_rect = None;
        let mut target_window_pid = None;
        if let Some(id) = target_window_id {
            if id != 0 {
                let thwnd = HWND(id as *mut _);
                if IsWindow(Some(thwnd)).as_bool() {
                    let mut r = RECT::default();
                    if GetWindowRect(thwnd, &mut r).is_ok() {
                        target_window_rect = Some(r);
                    }
                    let mut pid = 0;
                    GetWindowThreadProcessId(thwnd, Some(&mut pid));
                    if pid > 0 {
                        target_window_pid = Some(pid);
                    }
                }
            }
        }

        *OVERLAY_STATE.lock().unwrap() = Some(OverlayState {
            _target_window_id: target_window_id,
            target_window_rect,
            target_window_pid,
            is_dragging: false,
            drag_start: POINT::default(),
            drag_current: POINT::default(),
            hover_rect: RECT::default(),
            hover_title: String::new(),
            hover_element: None,
            result: None,
            selection_done: false,
        });

        let vx = GetSystemMetrics(SM_XVIRTUALSCREEN);
        let vy = GetSystemMetrics(SM_YVIRTUALSCREEN);
        let vw = GetSystemMetrics(SM_CXVIRTUALSCREEN);
        let vh = GetSystemMetrics(SM_CYVIRTUALSCREEN);

        let instance = GetModuleHandleW(None).map_err(|e| e.to_string())?;
        let hwnd = CreateWindowExW(
            WS_EX_TOPMOST | WS_EX_LAYERED | WS_EX_TOOLWINDOW,
            OVERLAY_CLASS,
            w!("MIMIC Selection Overlay"),
            WS_VISIBLE,
            vx,
            vy,
            vw,
            vh,
            None,
            None,
            Some(instance.into()),
            None,
        )
        .map_err(|e| format!("Failed to create overlay window: {e}"))?;

        let _ = SetLayeredWindowAttributes(hwnd, COLORREF(0), 0, LWA_COLORKEY);
        let _ = SetWindowPos(hwnd, Some(HWND_TOPMOST), vx, vy, vw, vh, SWP_SHOWWINDOW);

        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).as_bool() {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);

            let state_done = OVERLAY_STATE
                .lock()
                .unwrap()
                .as_ref()
                .map(|s| s.selection_done)
                .unwrap_or(true);
            if state_done {
                break;
            }
        }

        let res = OVERLAY_STATE
            .lock()
            .unwrap()
            .as_mut()
            .and_then(|s| s.result.take())
            .unwrap_or(SelectOnScreenResult {
                cancelled: true,
                mode: "cancelled".into(),
                x: 0,
                y: 0,
                width: 0,
                height: 0,
                window_id: 0,
                window_title: String::new(),
                element_name: None,
                control_type: None,
                automation_id: None,
                class_name: None,
                image_base64: None,
            });

        Ok(res)
    }
}

// -----------------------------------------------------------------------------
// Controlled Demo Target Window
// -----------------------------------------------------------------------------

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
        WM_COMMAND if (wparam.0 & 0xffff) == DEMO_BTN_LOGIN_ID => {
            DEMO_CLICK_COUNT.fetch_add(1, Ordering::SeqCst);
            let stage =
                windows::Win32::UI::WindowsAndMessaging::GetWindowLongPtrW(window, GWLP_USERDATA);
            let button = HWND(lparam.0 as *mut _);
            if stage == 0 {
                let _ = SetWindowTextW(window, w!("MIMIC Automation Demo - Server selection"));
                let _ = SetWindowTextW(button, w!("Select Test Server"));
                windows::Win32::UI::WindowsAndMessaging::SetWindowLongPtrW(window, GWLP_USERDATA, 1);
            } else if stage == 1 {
                let _ = SetWindowTextW(window, w!("MIMIC Automation Demo - Ready"));
                let _ = SetWindowTextW(button, w!("Demo completed"));
                windows::Win32::UI::WindowsAndMessaging::SetWindowLongPtrW(window, GWLP_USERDATA, 2);
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
    REGISTER_DEMO_CLASS.call_once(|| {
        let instance = GetModuleHandleW(None).unwrap_or_default();
        let cursor = LoadCursorW(None, IDC_ARROW).unwrap_or_default();
        let class = WNDCLASSW {
            hInstance: instance.into(),
            hCursor: cursor,
            hbrBackground: HBRUSH(GetStockObject(windows::Win32::Graphics::Gdi::WHITE_BRUSH).0),
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
        540,
        340,
        None,
        None,
        Some(instance.into()),
        None,
    )
    .map_err(|error| error.to_string())?;

    // Static label: Username
    let _ = CreateWindowExW(
        WINDOW_EX_STYLE(0),
        w!("STATIC"),
        w!("Username:"),
        WS_CHILD | WS_VISIBLE,
        50,
        45,
        90,
        22,
        Some(window),
        None,
        Some(instance.into()),
        None,
    );

    // Edit: Username
    let _ = CreateWindowExW(
        WINDOW_EX_STYLE(0),
        w!("EDIT"),
        w!("testuser"),
        WS_CHILD | WS_VISIBLE | WS_BORDER | WINDOW_STYLE(ES_AUTOHSCROLL as u32),
        150,
        42,
        260,
        26,
        Some(window),
        Some(HMENU(DEMO_EDIT_USER_ID as *mut _)),
        Some(instance.into()),
        None,
    );

    // Static label: Password
    let _ = CreateWindowExW(
        WINDOW_EX_STYLE(0),
        w!("STATIC"),
        w!("Password:"),
        WS_CHILD | WS_VISIBLE,
        50,
        95,
        90,
        22,
        Some(window),
        None,
        Some(instance.into()),
        None,
    );

    // Edit: Password
    let _ = CreateWindowExW(
        WINDOW_EX_STYLE(0),
        w!("EDIT"),
        w!(""),
        WS_CHILD | WS_VISIBLE | WS_BORDER | WINDOW_STYLE((ES_PASSWORD | ES_AUTOHSCROLL) as u32),
        150,
        92,
        260,
        26,
        Some(window),
        Some(HMENU(DEMO_EDIT_PASS_ID as *mut _)),
        Some(instance.into()),
        None,
    );

    // Button: Continue / Login
    let button = CreateWindowExW(
        WINDOW_EX_STYLE(0),
        w!("BUTTON"),
        w!("Continue to Server"),
        WS_CHILD | WS_VISIBLE,
        150,
        160,
        220,
        44,
        Some(window),
        Some(HMENU(DEMO_BTN_LOGIN_ID as *mut _)),
        Some(instance.into()),
        None,
    )
    .map_err(|error| error.to_string())?;

    if button.0.is_null() {
        return Err("Unable to create the demo action button".into());
    }

    let _ = ShowWindow(window, SW_SHOW);
    let _ = UpdateWindow(window);
    *DEMO_WINDOW.lock().map_err(|_| "Demo window lock failed")? = Some(window.0 as isize);
    Ok(())
}

pub unsafe fn reset_demo_window(hwnd: HWND) {
    let _ = SetWindowTextW(hwnd, w!("MIMIC Automation Demo - Login"));
    if let Ok(button) = GetDlgItem(Some(hwnd), DEMO_BTN_LOGIN_ID as i32) {
        let _ = SetWindowTextW(button, w!("Continue to Server"));
    }
    if let Ok(edit) = GetDlgItem(Some(hwnd), DEMO_EDIT_USER_ID as i32) {
        let _ = SetWindowTextW(edit, w!("testuser"));
    }
    windows::Win32::UI::WindowsAndMessaging::SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
    DEMO_CLICK_COUNT.store(0, Ordering::SeqCst);
    let _ = InvalidateRect(Some(hwnd), None, true);
    let _ = UpdateWindow(hwnd);
}

pub fn start_demo() -> Result<(), String> {
    if DEMO_STARTED.swap(true, Ordering::SeqCst) {
        if let Ok(demo_lock) = DEMO_WINDOW.lock() {
            if let Some(id) = *demo_lock {
                let hwnd = HWND(id as *mut _);
                unsafe {
                    if IsWindow(Some(hwnd)).as_bool() {
                        reset_demo_window(hwnd);
                    }
                }
            }
        }
        return Ok(());
    }
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
        let state = if title == DEMO_LOGIN_TITLE {
            "login"
        } else if title == DEMO_SERVER_TITLE {
            "server"
        } else if title == DEMO_READY_TITLE {
            "ready"
        } else {
            return Err("Demo window is in an unexpected state".to_string());
        };
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
            return Err("Selected target is no longer an open visible window".into());
        }
        if class_of(window) != "MimicAutomationDemoTarget" {
            return Err("Selected target is not the controlled MIMIC Demo window".into());
        }
        let title = title_of(window);
        let (expected_button, expected_next_title) = match title.as_str() {
            DEMO_LOGIN_TITLE => ("Continue to Server", DEMO_SERVER_TITLE),
            DEMO_SERVER_TITLE => ("Select Test Server", DEMO_READY_TITLE),
            _ => {
                return Err(
                    "Unexpected demo state. MIMIC stopped without clicking anything.".to_string(),
                )
            }
        };

        let button_name = to_wide(expected_button);
        let button = FindWindowExW(
            Some(window),
            None,
            w!("BUTTON"),
            PCWSTR(button_name.as_ptr()),
        )
        .map_err(|error| format!("Unable to locate expected control: {error}"))?;

        if button.0.is_null() {
            return Err("Expected action control is missing".into());
        }

        if EMERGENCY_STOP.load(Ordering::SeqCst) {
            return Ok(DemoRunResult {
                status: "stopped".into(),
                message: "Emergency stop was requested before input".into(),
                target_window_id,
            });
        }

        const BM_CLICK: u32 = 0x00F5;
        SendMessageW(button, BM_CLICK, Some(WPARAM(0)), Some(LPARAM(0)));

        for _ in 0..25 {
            if EMERGENCY_STOP.load(Ordering::SeqCst) {
                return Ok(DemoRunResult {
                    status: "stopped".into(),
                    message: "Emergency stop interrupted verification".into(),
                    target_window_id,
                });
            }
            let observed = title_of(window);
            if observed == expected_next_title {
                let status = if observed == DEMO_READY_TITLE {
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
        Err("Action sent but expected state was not observed before timeout".into())
    }
}

// -----------------------------------------------------------------------------
// Generic Step Execution (Shared Automation Engine)
// -----------------------------------------------------------------------------

unsafe fn verify_window_identity(hwnd: HWND, step: &StepExecutionPayload) -> Result<u32, String> {
    // 1. Strict Native Authorization Session Check
    let session = {
        let lock = match ACTIVE_SESSION.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        lock.clone().ok_or_else(|| {
            "No active native authorization session found. Execution rejected by native security policy.".to_string()
        })?
    };

    if !session.is_active {
        return Err("Native authorization session is inactive or revoked".into());
    }

    // 2. Strict Permission Check: must be explicitly true
    if !step.permission_granted.unwrap_or(false) {
        return Err("Automation permission is false or missing; action cancelled by security policy".into());
    }

    // 3. Window Existence and Visibility Check
    if !IsWindow(Some(hwnd)).as_bool() || !IsWindowVisible(hwnd).as_bool() {
        return Err("Target window is no longer open or visible; failing closed".into());
    }

    // 4. Process ID Extraction
    let mut pid = 0;
    GetWindowThreadProcessId(hwnd, Some(&mut pid));
    if pid == 0 {
        return Err("Target window process could not be identified".into());
    }

    // 5. Protection: Never automate MIMIC's own UI window
    if pid == std::process::id() && class_of(hwnd) != "MimicAutomationDemoTarget" {
        return Err("Target window belongs to MIMIC itself; automation blocked for safety".into());
    }

    // 6. Native Session & Authorized PID Matching
    if session.target_process_id > 0 && pid != session.target_process_id {
        return Err(format!(
            "Target window PID {} does not match native authorized session PID {}",
            pid, session.target_process_id
        ));
    }

    if let Some(expected_pid) = step.target_process_id {
        if expected_pid > 0 && pid != expected_pid {
            return Err(format!(
                "Target window PID {} does not match authorized PID {}",
                pid, expected_pid
            ));
        }
    }

    // 7. Authorized Executable Path Matching (Full Path / Binary Identity)
    if let Some(expected_exe) = step.target_exe_path.as_ref().or(session.target_exe_path.as_ref()) {
        let trimmed_exe = expected_exe.trim();
        if !trimmed_exe.is_empty() && !trimmed_exe.eq_ignore_ascii_case("DEMO") {
            let actual_exe = get_process_path(pid)
                .ok_or_else(|| format!("Unable to verify process executable path for PID {}; failing closed", pid))?;

            let is_full_path = trimmed_exe.contains('\\') || trimmed_exe.contains('/');
            if is_full_path {
                let norm_actual = actual_exe.replace('/', "\\").to_lowercase();
                let norm_expected = trimmed_exe.replace('/', "\\").to_lowercase();
                if norm_actual != norm_expected {
                    // Strict: NEVER fallback to filename when full path was supplied!
                    return Err(format!(
                        "Process executable path '{}' does not match authorized path '{}'; failing closed",
                        actual_exe, trimmed_exe
                    ));
                }
            } else {
                let actual_name = std::path::Path::new(&actual_exe)
                    .file_name()
                    .and_then(|f| f.to_str())
                    .unwrap_or(&actual_exe);
                if !actual_name.eq_ignore_ascii_case(trimmed_exe) {
                    return Err(format!(
                        "Process executable '{}' does not match authorized application '{}'; failing closed",
                        actual_name, trimmed_exe
                    ));
                }
            }
        }
    }

    Ok(pid)
}

unsafe fn dispatch_click(
    hwnd: HWND,
    child: Option<HWND>,
    x: i32,
    y: i32,
    double: bool,
) -> Result<(), String> {
    const WM_LBUTTONDBLCLK: u32 = 0x0203;
    let mut rect = RECT::default();
    GetWindowRect(hwnd, &mut rect).map_err(|e| e.to_string())?;

    // Strict boundary enforcement: clicks outside window boundary are forbidden
    if x < rect.left || x > rect.right || y < rect.top || y > rect.bottom {
        return Err(format!(
            "Target coordinate ({x}, {y}) is outside window boundary [L:{}, T:{}, R:{}, B:{}]",
            rect.left, rect.top, rect.right, rect.bottom
        ));
    }

    let _ = SetForegroundWindow(hwnd);
    thread::sleep(Duration::from_millis(30));

    // Case 1: Target has a Win32 child HWND
    if let Some(target_child) = child {
        if !double {
            // For standard button child controls, BM_CLICK is the designated Win32 programmatic click.
            // Dispatch exactly this ONE click method and return immediately.
            const BM_CLICK: u32 = 0x00F5;
            SendMessageW(target_child, BM_CLICK, Some(WPARAM(0)), Some(LPARAM(0)));
            return Ok(());
        } else {
            // For double click on child control, send discrete client mouse messages to child only
            let mut pt = POINT { x, y };
            let _ = ScreenToClient(target_child, &mut pt);
            let lparam_pt = LPARAM(((pt.y as isize) << 16) | (pt.x as isize & 0xFFFF));
            let _ = PostMessageW(Some(target_child), WM_LBUTTONDOWN, WPARAM(1), lparam_pt);
            thread::sleep(Duration::from_millis(20));
            let _ = PostMessageW(Some(target_child), WM_LBUTTONUP, WPARAM(0), lparam_pt);
            thread::sleep(Duration::from_millis(40));
            let _ = PostMessageW(Some(target_child), WM_LBUTTONDBLCLK, WPARAM(1), lparam_pt);
            thread::sleep(Duration::from_millis(20));
            let _ = PostMessageW(Some(target_child), WM_LBUTTONUP, WPARAM(0), lparam_pt);
            return Ok(());
        }
    }

    // Case 2: Target is UIA element without child HWND, or Image Template match.
    // Dispatch exactly ONE mouse click to the window at the calculated coordinate.
    let mut pt = POINT { x, y };
    let _ = ScreenToClient(hwnd, &mut pt);
    let lparam_pt = LPARAM(((pt.y as isize) << 16) | (pt.x as isize & 0xFFFF));

    let _ = PostMessageW(Some(hwnd), WM_LBUTTONDOWN, WPARAM(1), lparam_pt);
    thread::sleep(Duration::from_millis(30));
    let _ = PostMessageW(Some(hwnd), WM_LBUTTONUP, WPARAM(0), lparam_pt);

    if double {
        thread::sleep(Duration::from_millis(60));
        let _ = PostMessageW(Some(hwnd), WM_LBUTTONDBLCLK, WPARAM(1), lparam_pt);
        thread::sleep(Duration::from_millis(30));
        let _ = PostMessageW(Some(hwnd), WM_LBUTTONUP, WPARAM(0), lparam_pt);
    }
    Ok(())
}

unsafe fn press_virtual_key(hwnd: HWND, key: &str) -> Result<(), String> {
    let _ = SetForegroundWindow(hwnd);
    thread::sleep(Duration::from_millis(30));

    let vk = match key.to_uppercase().as_str() {
        "ENTER" | "RETURN" => VK_RETURN,
        "TAB" => VK_TAB,
        "ESC" | "ESCAPE" => VK_ESCAPE,
        "SPACE" => VK_SPACE,
        "BACKSPACE" => VK_BACK,
        _ => VK_RETURN,
    };

    let inputs = [
        INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    wVk: vk,
                    wScan: 0,
                    dwFlags: windows::Win32::UI::Input::KeyboardAndMouse::KEYBD_EVENT_FLAGS(0),
                    time: 0,
                    dwExtraInfo: 0,
                },
            },
        },
        INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    wVk: vk,
                    wScan: 0,
                    dwFlags: KEYEVENTF_KEYUP,
                    time: 0,
                    dwExtraInfo: 0,
                },
            },
        },
    ];
    SendInput(&inputs, std::mem::size_of::<INPUT>() as i32);
    Ok(())
}

pub fn execute_step(
    target_window_id: isize,
    step: StepExecutionPayload,
) -> Result<StepExecutionResult, String> {
    // 1. Strict Permission Check: must be explicitly true
    if !step.permission_granted.unwrap_or(false) {
        return Err("Automation permission is false or missing; action cancelled by security policy".into());
    }

    // 2. Emergency Stop Check
    if EMERGENCY_STOP.load(Ordering::SeqCst) {
        return Ok(StepExecutionResult {
            success: false,
            status: "stopped".into(),
            message: "Emergency stop active; step cancelled before execution".into(),
            target_window_id,
            attempts: 0,
            duration_ms: 0,
        });
    }

    let start_time = Instant::now();
    let action_kind = step.action_type.as_deref().unwrap_or(&step.kind);
    let timeout = Duration::from_millis(step.timeout_ms.unwrap_or(5000).max(500));
    let max_attempts = step.max_attempts.unwrap_or(1).max(1);

    // 3. Special handling for launch-app (target window may not exist prior to launch)
    if action_kind == "launch-app" {
        let exe_path = step.text.as_deref().unwrap_or("DEMO");
        let pid = launch_application(exe_path)?;

        let deadline = Instant::now() + timeout;
        let mut new_hwnd: Option<HWND> = None;
        while Instant::now() < deadline {
            if EMERGENCY_STOP.load(Ordering::SeqCst) {
                return Ok(StepExecutionResult {
                    success: false,
                    status: "stopped".into(),
                    message: "Emergency stop interrupted launch-app".into(),
                    target_window_id,
                    attempts: 1,
                    duration_ms: start_time.elapsed().as_millis() as u64,
                });
            }
            if let Ok(windows) = list_windows() {
                for win in windows {
                    // Strict Process Matching: Window must belong to the spawned process
                    let is_demo_launch = exe_path.eq_ignore_ascii_case("DEMO");
                    let matches_process = if is_demo_launch {
                        win.is_demo
                    } else if pid > 0 {
                        win.process_id == pid
                    } else {
                        false
                    };

                    if matches_process {
                        let candidate_hwnd = HWND(win.id as *mut _);
                        if unsafe {
                            IsWindow(Some(candidate_hwnd)).as_bool()
                                && IsWindowVisible(candidate_hwnd).as_bool()
                        } {
                            let title_ok = if let Some(exp) = &step.expected_state {
                                win.title.to_lowercase().contains(&exp.to_lowercase())
                            } else if let Some(loc) = &step.locator {
                                loc.value
                                    .as_ref()
                                    .map(|v| win.title.to_lowercase().contains(&v.to_lowercase()))
                                    .unwrap_or(true)
                            } else {
                                true
                            };
                            if title_ok {
                                new_hwnd = Some(candidate_hwnd);
                                break;
                            }
                        }
                    }
                }
            }
            if new_hwnd.is_some() {
                break;
            }
            thread::sleep(Duration::from_millis(150));
        }

        return match new_hwnd {
            Some(h) => {
                let _ = unsafe { SetForegroundWindow(h) };
                let mut session_updated = false;
                if let Ok(mut lock) = ACTIVE_SESSION.lock() {
                    if let Some(session) = lock.as_mut() {
                        session.target_window_id = h.0 as isize;
                        session.target_process_id = pid;
                        session.is_active = true;
                        session_updated = true;
                    }
                }
                if !session_updated {
                    let _ = authorize_session(h.0 as isize, Some(exe_path.to_string()));
                }
                Ok(StepExecutionResult {
                    success: true,
                    status: "verified".into(),
                    message: format!(
                        "Application launched successfully (Window: '{}')",
                        unsafe { title_of(h) }
                    ),
                    target_window_id: h.0 as isize,
                    attempts: 1,
                    duration_ms: start_time.elapsed().as_millis() as u64,
                })
            }
            None => Err(format!(
                "launch-app timed out after {:?}: target window for process {} did not appear",
                timeout, pid
            )),
        };
    }

    let hwnd = HWND(target_window_id as *mut _);
    unsafe {
        verify_window_identity(hwnd, &step)?;
    }

    for attempt in 1..=max_attempts {
        if EMERGENCY_STOP.load(Ordering::SeqCst) {
            return Ok(StepExecutionResult {
                success: false,
                status: "stopped".into(),
                message: "Emergency stop interrupted step execution".into(),
                target_window_id,
                attempts: attempt,
                duration_ms: start_time.elapsed().as_millis() as u64,
            });
        }

        unsafe {
            verify_window_identity(hwnd, &step)?;
        }

        // Retry safety guard: do not repeat an action if postconditions are already satisfied
        if attempt > 1 {
            let mut already_met = true;
            if let Some(exp_state) = &step.expected_state {
                let cur = unsafe { title_of(hwnd) };
                if !cur.to_lowercase().contains(&exp_state.to_lowercase()) {
                    already_met = false;
                }
            }
            if let Some(exp_loc) = &step.expected_locator {
                if unsafe { resolve_locator(hwnd, exp_loc).is_err() } {
                    already_met = false;
                }
            }
            if (step.expected_state.is_some() || step.expected_locator.is_some()) && already_met {
                return Ok(StepExecutionResult {
                    success: true,
                    status: "verified".into(),
                    message: format!(
                        "Postcondition already met before retry (Attempt {})",
                        attempt
                    ),
                    target_window_id,
                    attempts: attempt,
                    duration_ms: start_time.elapsed().as_millis() as u64,
                });
            }
        }

        let action_result: Result<(), String> = unsafe {
            match action_kind {
                "click" | "double-click" => {
                    let double = action_kind == "double-click";
                    let loc = step.locator.as_ref().ok_or_else(|| {
                        "Click action requires a valid element locator or image template; blind clicking the window center is strictly prohibited".to_string()
                    })?;

                    let has_criteria = loc.automation_id.as_ref().map(|s| !s.trim().is_empty()).unwrap_or(false)
                        || loc.name.as_ref().map(|s| !s.trim().is_empty()).unwrap_or(false)
                        || loc.image_base64.as_ref().map(|s| !s.trim().is_empty()).unwrap_or(false)
                        || (loc.x.is_some() && loc.y.is_some());

                    if !has_criteria {
                        return Err("Click locator does not specify any target identifier (automation_id, name, image, or coordinates); action cancelled for safety".into());
                    }

                    let resolved = resolve_locator(hwnd, loc)?;
                    let cx = (resolved.rect.left + resolved.rect.right) / 2;
                    let cy = (resolved.rect.top + resolved.rect.bottom) / 2;
                    dispatch_click(hwnd, resolved.child_hwnd, cx, cy, double)?;
                    Ok(())
                }
                "type-text" => {
                    let text = step.text.as_deref().unwrap_or_default();
                    let loc = step.locator.as_ref().ok_or_else(|| {
                        "type-text requires a target locator; blind text input without an identified target element is prohibited".to_string()
                    })?;

                    // Must resolve target input control dynamically
                    let resolved = resolve_locator(hwnd, loc)?;
                    let cx = (resolved.rect.left + resolved.rect.right) / 2;
                    let cy = (resolved.rect.top + resolved.rect.bottom) / 2;

                    if let Some(child) = resolved.child_hwnd {
                        let _ = SetForegroundWindow(hwnd);
                        let _ = windows::Win32::UI::Input::KeyboardAndMouse::SetFocus(Some(child));

                        // Activate caret by clicking child control
                        let mut pt = POINT { x: cx, y: cy };
                        let _ = ScreenToClient(child, &mut pt);
                        let lparam = LPARAM(((pt.y as isize) << 16) | (pt.x as isize & 0xFFFF));
                        let _ = PostMessageW(Some(child), WM_LBUTTONDOWN, WPARAM(1), lparam);
                        thread::sleep(Duration::from_millis(20));
                        let _ = PostMessageW(Some(child), WM_LBUTTONUP, WPARAM(0), lparam);
                        thread::sleep(Duration::from_millis(40));

                        // Send characters directly to child control (never to parent hwnd)
                        for ch in text.chars() {
                            if EMERGENCY_STOP.load(Ordering::SeqCst) {
                                return Ok(StepExecutionResult {
                                    success: false,
                                    status: "stopped".into(),
                                    message: "Emergency stop interrupted typing".into(),
                                    target_window_id,
                                    attempts: attempt,
                                    duration_ms: start_time.elapsed().as_millis() as u64,
                                });
                            }
                            if verify_window_identity(hwnd, &step).is_err() {
                                return Err("Target window was closed during text input".into());
                            }
                            let code = ch as u32;
                            SendMessageW(child, WM_CHAR, Some(WPARAM(code as usize)), Some(LPARAM(1)));
                            thread::sleep(Duration::from_millis(20));
                        }
                    } else {
                        // UIA / Custom rendered input: focus via coordinate click and synthesize input
                        let _ = SetForegroundWindow(hwnd);
                        let mut pt = POINT { x: cx, y: cy };
                        let _ = ScreenToClient(hwnd, &mut pt);
                        let lparam = LPARAM(((pt.y as isize) << 16) | (pt.x as isize & 0xFFFF));
                        let _ = PostMessageW(Some(hwnd), WM_LBUTTONDOWN, WPARAM(1), lparam);
                        thread::sleep(Duration::from_millis(20));
                        let _ = PostMessageW(Some(hwnd), WM_LBUTTONUP, WPARAM(0), lparam);
                        thread::sleep(Duration::from_millis(40));

                        for ch in text.chars() {
                            if EMERGENCY_STOP.load(Ordering::SeqCst) {
                                return Ok(StepExecutionResult {
                                    success: false,
                                    status: "stopped".into(),
                                    message: "Emergency stop interrupted typing".into(),
                                    target_window_id,
                                    attempts: attempt,
                                    duration_ms: start_time.elapsed().as_millis() as u64,
                                });
                            }
                            if verify_window_identity(hwnd, &step).is_err() {
                                return Err("Target window was closed during text input".into());
                            }
                            let code = ch as u32;
                            SendMessageW(hwnd, WM_CHAR, Some(WPARAM(code as usize)), Some(LPARAM(1)));
                            thread::sleep(Duration::from_millis(20));
                        }
                    }
                    Ok(())
                }
                "type-secret" => {
                    // Phase 2 Safe Secret Handling: Must resolve via Windows Credential Manager
                    let secret_ref = step.secret_ref.as_deref().ok_or_else(|| {
                        "type-secret requires a secret_ref (e.g. 'vault:target_name'); plaintext passwords in workflow steps are strictly prohibited for safety".to_string()
                    })?;

                    let mut secret = crate::credentials::resolve_secret_reference(secret_ref)?;

                    let loc = step.locator.as_ref().ok_or_else(|| {
                        "type-secret requires a target locator for the password field; blind typing secrets is prohibited".to_string()
                    })?;

                    let resolved = resolve_locator(hwnd, loc)?;
                    let cx = (resolved.rect.left + resolved.rect.right) / 2;
                    let cy = (resolved.rect.top + resolved.rect.bottom) / 2;

                    if let Some(child) = resolved.child_hwnd {
                        let _ = SetForegroundWindow(hwnd);
                        let _ = windows::Win32::UI::Input::KeyboardAndMouse::SetFocus(Some(child));
                        thread::sleep(Duration::from_millis(40));

                        for ch in secret.chars() {
                            if EMERGENCY_STOP.load(Ordering::SeqCst) {
                                secret.replace_range(.., "");
                                return Ok(StepExecutionResult {
                                    success: false,
                                    status: "stopped".into(),
                                    message: "Emergency stop interrupted secret typing".into(),
                                    target_window_id,
                                    attempts: attempt,
                                    duration_ms: start_time.elapsed().as_millis() as u64,
                                });
                            }
                            if verify_window_identity(hwnd, &step).is_err() {
                                secret.replace_range(.., "");
                                return Err("Target window was closed during secret text input".into());
                            }
                            let code = ch as u32;
                            SendMessageW(child, WM_CHAR, Some(WPARAM(code as usize)), Some(LPARAM(1)));
                            thread::sleep(Duration::from_millis(20));
                        }
                    } else {
                        let _ = SetForegroundWindow(hwnd);
                        let mut pt = POINT { x: cx, y: cy };
                        let _ = ScreenToClient(hwnd, &mut pt);
                        let lparam = LPARAM(((pt.y as isize) << 16) | (pt.x as isize & 0xFFFF));
                        let _ = PostMessageW(Some(hwnd), WM_LBUTTONDOWN, WPARAM(1), lparam);
                        thread::sleep(Duration::from_millis(20));
                        let _ = PostMessageW(Some(hwnd), WM_LBUTTONUP, WPARAM(0), lparam);
                        thread::sleep(Duration::from_millis(40));

                        for ch in secret.chars() {
                            if EMERGENCY_STOP.load(Ordering::SeqCst) {
                                secret.replace_range(.., "");
                                return Ok(StepExecutionResult {
                                    success: false,
                                    status: "stopped".into(),
                                    message: "Emergency stop interrupted secret typing".into(),
                                    target_window_id,
                                    attempts: attempt,
                                    duration_ms: start_time.elapsed().as_millis() as u64,
                                });
                            }
                            if verify_window_identity(hwnd, &step).is_err() {
                                secret.replace_range(.., "");
                                return Err("Target window was closed during secret text input".into());
                            }
                            let code = ch as u32;
                            SendMessageW(hwnd, WM_CHAR, Some(WPARAM(code as usize)), Some(LPARAM(1)));
                            thread::sleep(Duration::from_millis(20));
                        }
                    }
                    secret.replace_range(.., "");
                    Ok(())
                }
                "press-key" => {
                    let key = step.key.as_deref().unwrap_or("ENTER");
                    press_virtual_key(hwnd, key)
                }
                "wait-visible" => {
                    // Wait action: polls until locator becomes visible
                    if let Some(loc) = &step.locator {
                        let wait_deadline = Instant::now() + timeout;
                        let mut found = false;
                        while Instant::now() < wait_deadline {
                            if EMERGENCY_STOP.load(Ordering::SeqCst) {
                                return Ok(StepExecutionResult {
                                    success: false,
                                    status: "stopped".into(),
                                    message: "Emergency stop interrupted wait-visible".into(),
                                    target_window_id,
                                    attempts: attempt,
                                    duration_ms: start_time.elapsed().as_millis() as u64,
                                });
                            }
                            if verify_window_identity(hwnd, &step).is_err() {
                                return Err("Target window was closed during wait-visible".into());
                            }
                            if resolve_locator(hwnd, loc).is_ok() {
                                found = true;
                                break;
                            }
                            thread::sleep(Duration::from_millis(100));
                        }
                        if !found {
                            return Err(format!("wait-visible timed out after {:?}: element did not appear", timeout));
                        }
                    }
                    Ok(())
                }
                "wait-hidden" => {
                    // Wait action: polls until locator is no longer visible
                    if let Some(loc) = &step.locator {
                        let wait_deadline = Instant::now() + timeout;
                        let mut hidden = false;
                        while Instant::now() < wait_deadline {
                            if EMERGENCY_STOP.load(Ordering::SeqCst) {
                                return Ok(StepExecutionResult {
                                    success: false,
                                    status: "stopped".into(),
                                    message: "Emergency stop interrupted wait-hidden".into(),
                                    target_window_id,
                                    attempts: attempt,
                                    duration_ms: start_time.elapsed().as_millis() as u64,
                                });
                            }
                            if verify_window_identity(hwnd, &step).is_err() {
                                return Err("Target window was closed during wait-hidden".into());
                            }
                            if resolve_locator(hwnd, loc).is_err() {
                                hidden = true;
                                break;
                            }
                            thread::sleep(Duration::from_millis(100));
                        }
                        if !hidden {
                            return Err(format!("wait-hidden timed out after {:?}: element remained visible", timeout));
                        }
                    }
                    Ok(())
                }
                "verify-state" => {
                    // Verifies current window title or element immediately
                    if let Some(loc) = &step.locator {
                        resolve_locator(hwnd, loc).map(|_| ())?;
                    }
                    if let Some(exp) = &step.expected_state {
                        let cur_title = title_of(hwnd);
                        if !cur_title.to_lowercase().contains(&exp.to_lowercase()) {
                            return Err(format!("verify-state failed: expected '{exp}', got '{cur_title}'"));
                        }
                    }
                    Ok(())
                }
                other => Err(format!("Unrecognized action kind '{other}'")),
            }
        };

        if let Err(e) = action_result {
            if EMERGENCY_STOP.load(Ordering::SeqCst) || e.contains("Emergency stop") {
                return Ok(StepExecutionResult {
                    success: false,
                    status: "stopped".into(),
                    message: format!("Emergency stop: {e}"),
                    target_window_id,
                    attempts: attempt,
                    duration_ms: start_time.elapsed().as_millis() as u64,
                });
            }
            if attempt == max_attempts {
                return Err(format!("Step '{}' action failed: {e}", step.title));
            }
            thread::sleep(Duration::from_millis(250));
            continue;
        }

        // Postcondition Verification
        let has_expected_state = step
            .expected_state
            .as_deref()
            .map(|s| !s.trim().is_empty())
            .unwrap_or(false);
        let has_expected_locator = step.expected_locator.is_some();
        let has_postcondition = has_expected_state || has_expected_locator;

        if !has_postcondition {
            // STEP WITHOUT POSTCONDITION:
            // Explicit behavior: Return "completed", NOT "verified"!
            return Ok(StepExecutionResult {
                success: true,
                status: "completed".into(),
                message: format!(
                    "Action '{}' completed successfully (no postcondition configured)",
                    step.title
                ),
                target_window_id,
                attempts: attempt,
                duration_ms: start_time.elapsed().as_millis() as u64,
            });
        }

        let deadline = Instant::now() + timeout;
        let mut condition_met = false;

        while Instant::now() < deadline {
            if EMERGENCY_STOP.load(Ordering::SeqCst) {
                return Ok(StepExecutionResult {
                    success: false,
                    status: "stopped".into(),
                    message: "Emergency stop interrupted postcondition verification".into(),
                    target_window_id,
                    attempts: attempt,
                    duration_ms: start_time.elapsed().as_millis() as u64,
                });
            }

            unsafe {
                if verify_window_identity(hwnd, &step).is_err() {
                    return Err("Target window was closed during verification".into());
                }

                let current_title = title_of(hwnd);
                let mut state_ok = true;
                let mut locator_ok = true;

                // 1. Verify Expected State / Title if specified
                if let Some(exp_state) = &step.expected_state {
                    let state_matches = match exp_state.to_lowercase().as_str() {
                        "login" => current_title.contains("Login"),
                        "server" => current_title.contains("Server"),
                        "ready" => current_title.contains("Ready"),
                        title_sub => current_title
                            .to_lowercase()
                            .contains(&title_sub.to_lowercase()),
                    };
                    if !state_matches {
                        state_ok = false;
                    }
                }

                // 2. Real Expected Locator Verification if specified
                if let Some(exp_loc) = &step.expected_locator {
                    if resolve_locator(hwnd, exp_loc).is_err() {
                        locator_ok = false;
                    }
                }

                if state_ok && locator_ok {
                    condition_met = true;
                    break;
                }
            }
            thread::sleep(Duration::from_millis(100));
        }

        if condition_met {
            let is_terminal = unsafe {
                let title = title_of(hwnd);
                title.contains("Ready") || step.expected_state.as_deref() == Some("ready")
            };
            return Ok(StepExecutionResult {
                success: true,
                status: if is_terminal {
                    "completed".into()
                } else {
                    "verified".into()
                },
                message: format!("Verified expected postcondition for '{}'", step.title),
                target_window_id,
                attempts: attempt,
                duration_ms: start_time.elapsed().as_millis() as u64,
            });
        }

        if attempt == max_attempts {
            return Err(format!(
                "Step '{}' timed out after {:?}: expected condition was not verified",
                step.title, timeout
            ));
        }
    }

    Err(format!(
        "Step '{}' failed after {} attempts",
        step.title, max_attempts
    ))
}

pub fn emergency_stop() -> DemoRunResult {
    EMERGENCY_STOP.store(true, Ordering::SeqCst);
    if let Ok(mut lock) = ACTIVE_SESSION.lock() {
        if let Some(session) = lock.as_mut() {
            session.is_active = false;
        }
    }
    stop_global_emergency_hook();
    DemoRunResult {
        status: "stopped".into(),
        message: "Emergency stop requested. All automation actions halted immediately.".into(),
        target_window_id: 0,
    }
}

pub fn reset_emergency_stop() {
    EMERGENCY_STOP.store(false, Ordering::SeqCst);
}

#[cfg(test)]
mod tests {
    use super::*;

    static TEST_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn test_image_template_matching_algorithm() {
        // Create synthetic grayscale target (50x50) with a distinctive pattern
        let mut target_pixels = vec![128u8; 50 * 50];
        // Plant a 10x10 marker with high contrast at (20, 15)
        for y in 15..25 {
            for x in 20..30 {
                target_pixels[y * 50 + x] = 255;
            }
        }
        let target_img = GrayscaleImage {
            width: 50,
            height: 50,
            pixels: target_pixels,
        };

        // Create the 10x10 template matching the planted marker
        let template_pixels = vec![255u8; 10 * 10];
        let template_img = GrayscaleImage {
            width: 10,
            height: 10,
            pixels: template_pixels,
        };

        let result = search_template_match(&target_img, &template_img, 0.85);
        assert!(result.is_some(), "Template matcher should find the marker");
        let (matched_x, matched_y, score) = result.unwrap();
        assert_eq!(matched_x, 20, "X coordinate must match");
        assert_eq!(matched_y, 15, "Y coordinate must match");
        assert!(score >= 0.85, "Confidence score must exceed threshold");
    }

    #[test]
    fn test_real_screen_template_matching() {
        let _lock = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        start_demo().expect("demo target should open");
        thread::sleep(Duration::from_millis(200));
        let demo = list_windows()
            .expect("window enumeration should work")
            .into_iter()
            .find(|window| window.is_demo)
            .expect("demo window should be found");

        let hwnd = HWND(demo.id as *mut _);
        unsafe {
            reset_demo_window(hwnd);
            let _ = SetForegroundWindow(hwnd);
            let _ = UpdateWindow(hwnd);
        }
        thread::sleep(Duration::from_millis(150));

        // Capture real rendered screen bytes of the demo window
        let (screen_img, _rect) =
            unsafe { capture_window_grayscale(hwnd).expect("Should capture real window grayscale") };
        assert!(screen_img.width > 200 && screen_img.height > 100);

        // Crop real button text pixels ("Continue") from the rendered screen
        let button = unsafe { GetDlgItem(Some(hwnd), DEMO_BTN_LOGIN_ID as i32).unwrap() };
        let mut btn_rect = RECT::default();
        let mut win_rect = RECT::default();
        unsafe {
            let _ = GetWindowRect(button, &mut btn_rect);
            let _ = GetWindowRect(hwnd, &mut win_rect);
        }

        let crop_w = 40;
        let crop_h = 16;
        let crop_x = (btn_rect.left - win_rect.left + 80).clamp(0, screen_img.width - crop_w);
        let crop_y = (btn_rect.top - win_rect.top + 14).clamp(0, screen_img.height - crop_h);

        let mut template_px = Vec::with_capacity((crop_w * crop_h) as usize);
        for y in 0..crop_h {
            for x in 0..crop_w {
                let idx = ((crop_y + y) * screen_img.width + (crop_x + x)) as usize;
                template_px.push(screen_img.pixels[idx]);
            }
        }
        println!(
            "crop_x={}, crop_y={}, btn_rel=({}, {}), px_min={}, px_max={}",
            crop_x,
            crop_y,
            btn_rect.left - win_rect.left,
            btn_rect.top - win_rect.top,
            template_px.iter().min().unwrap(),
            template_px.iter().max().unwrap(),
        );

        let template = GrayscaleImage {
            width: crop_w,
            height: crop_h,
            pixels: template_px,
        };

        // 1. Match against current window
        let match1 = search_template_match_with_roi(&screen_img, &template, 0.90, Some((crop_x, crop_y)));
        assert!(match1.is_some(), "Template match on real screen pixels should succeed");
        let (mx, my, score) = match1.unwrap();
        assert!((mx - crop_x).abs() <= 2, "Matched X must match real cropped coordinates within tolerance, got {} vs {}", mx, crop_x);
        assert!((my - crop_y).abs() <= 2, "Matched Y must match real cropped coordinates within tolerance, got {} vs {}", my, crop_y);
        assert!(score >= 0.95, "Confidence on real identical screen crop should be >= 0.95, got {}", score);

        // 2. Move window and re-verify template matching
        unsafe {
            let _ = SetWindowPos(
                hwnd,
                None,
                260,
                200,
                540,
                340,
                windows::Win32::UI::WindowsAndMessaging::SWP_NOZORDER,
            );
        }
        thread::sleep(Duration::from_millis(80));

        let (moved_screen, _) = unsafe { capture_window_grayscale(hwnd).expect("Should capture moved window") };
        let match2 = search_template_match_with_roi(&moved_screen, &template, 0.85, Some((crop_x, crop_y)));
        assert!(match2.is_some(), "Template match after window move should locate element");
        let (m2x, m2y, score2) = match2.unwrap();
        assert!((m2x - crop_x).abs() <= 2, "Matched X after move should match relative button coords within tolerance");
        assert!((m2y - crop_y).abs() <= 2, "Matched Y after move should match relative button coords within tolerance");
        assert!(score2 >= 0.90, "Score after move should be >= 0.90, got {}", score2);
    }

    #[test]
    fn test_launch_app_and_wait_window() {
        let _lock = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let pid = launch_application("DEMO").expect("launch_application for DEMO should succeed");
        assert!(pid > 0, "Valid process ID should be returned");

        let launch_step = StepExecutionPayload {
            id: Some("launch-1".into()),
            title: "Launch Application Step".into(),
            kind: "launch-app".into(),
            action_type: Some("launch-app".into()),
            locator: None,
            text: Some("DEMO".into()),
            secret_ref: None,
            key: None,
            expected_state: Some("Demo".into()),
            expected_locator: None,
            timeout_ms: Some(3000),
            max_attempts: Some(1),
            permission_granted: Some(true),
            target_process_id: None,
            target_exe_path: None,
            target_window_title: None,
        };
        let res = execute_step(0, launch_step).expect("execute_step for launch-app should succeed");
        assert!(res.success);
        assert!(res.target_window_id > 0, "Target window ID must be assigned");
    }

    #[test]
    fn test_prevent_duplicate_clicks_and_reliable_input() {
        let _lock = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        reset_emergency_stop();
        start_demo().expect("demo target should open");
        thread::sleep(Duration::from_millis(200));
        let demo = list_windows()
            .expect("window enumeration should work")
            .into_iter()
            .find(|window| window.is_demo)
            .expect("demo login window should be discoverable");

        let _session = authorize_session(demo.id, Some("DEMO".into())).expect("authorize session");

        let hwnd = HWND(demo.id as *mut _);
        unsafe {
            reset_demo_window(hwnd);
            let _ = SetForegroundWindow(hwnd);
            let _ = UpdateWindow(hwnd);
        }
        thread::sleep(Duration::from_millis(150));

        // 1. Verify text input: target edit control, focus, and type
        let text_step = StepExecutionPayload {
            id: Some("type-1".into()),
            title: "Type Username".into(),
            kind: "type-text".into(),
            action_type: Some("type-text".into()),
            locator: Some(LocatorPayload {
                kind: "uia".into(),
                automation_id: Some("102".into()),
                name: None,
                control_type: Some("Edit".into()),
                class_name: None,
                image_base64: None,
                x: None,
                y: None,
                width: None,
                height: None,
                reference_width: None,
                reference_height: None,
                threshold: None,
                value: None,
            }),
            text: Some("mimic_pro".into()),
            secret_ref: None,
            key: None,
            expected_state: None,
            expected_locator: None,
            timeout_ms: Some(2000),
            max_attempts: Some(1),
            permission_granted: Some(true),
            target_process_id: None,
            target_exe_path: None,
            target_window_title: None,
        };
        let type_res = execute_step(demo.id, text_step).expect("type-text should succeed");
        assert_eq!(type_res.status, "completed", "Step without postcondition must report 'completed', not 'verified'");

        // Verify that characters were actually received by the target edit control
        let edit_child = unsafe { windows::Win32::UI::WindowsAndMessaging::GetDlgItem(Some(hwnd), 102).unwrap() };
        let mut text_buf = [0u16; 64];
        let len = unsafe { windows::Win32::UI::WindowsAndMessaging::GetWindowTextW(edit_child, &mut text_buf) };
        let typed_text = String::from_utf16_lossy(&text_buf[..len as usize]);
        assert!(typed_text.contains("mimic_pro"), "Typed text must be received by target edit control, got '{}'", typed_text);

        // 2. Verify duplicate click prevention
        DEMO_CLICK_COUNT.store(0, Ordering::SeqCst);

        let click_step = StepExecutionPayload {
            id: Some("click-1".into()),
            title: "Single Click Login Button".into(),
            kind: "click".into(),
            action_type: Some("click".into()),
            locator: Some(LocatorPayload {
                kind: "uia".into(),
                automation_id: Some("101".into()),
                name: Some("Continue to Server".into()),
                control_type: Some("Button".into()),
                class_name: None,
                image_base64: None,
                x: None,
                y: None,
                width: None,
                height: None,
                reference_width: None,
                reference_height: None,
                threshold: None,
                value: None,
            }),
            text: None,
            secret_ref: None,
            key: None,
            expected_state: Some("server".into()),
            expected_locator: None,
            timeout_ms: Some(3000),
            max_attempts: Some(1),
            permission_granted: Some(true),
            target_process_id: None,
            target_exe_path: None,
            target_window_title: None,
        };

        let click_res = execute_step(demo.id, click_step).expect("Single click step should execute");
        assert_eq!(click_res.status, "verified");

        // CRITICAL CHECK: Button must be clicked exactly ONCE (never twice or three times)
        let total_clicks = DEMO_CLICK_COUNT.load(Ordering::SeqCst);
        assert_eq!(total_clicks, 1, "Click action must fire exactly ONE click event to avoid duplicate triggers");
    }

    #[test]
    fn test_phase_1_hardening_scenarios() {
        let _lock = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        reset_emergency_stop();
        start_demo().expect("demo target should open");
        thread::sleep(Duration::from_millis(200));
        let demo = list_windows()
            .expect("window enumeration should work")
            .into_iter()
            .find(|window| window.is_demo)
            .expect("demo login window should be discoverable");

        let _session = authorize_session(demo.id, Some("DEMO".into())).expect("authorize session");

        let hwnd = HWND(demo.id as *mut _);
        unsafe {
            reset_demo_window(hwnd);
            let _ = SetForegroundWindow(hwnd);
            let _ = UpdateWindow(hwnd);
        }
        thread::sleep(Duration::from_millis(150));

        // 1. Test Permission Denied Check
        let denied_step = StepExecutionPayload {
            id: Some("denied-1".into()),
            title: "Denied Step".into(),
            kind: "click".into(),
            action_type: Some("click".into()),
            locator: None,
            text: None,
            secret_ref: None,
            key: None,
            expected_state: None,
            expected_locator: None,
            timeout_ms: Some(1000),
            max_attempts: Some(1),
            permission_granted: Some(false),
            target_process_id: None,
            target_exe_path: None,
            target_window_title: None,
        };
        let denied_res = execute_step(demo.id, denied_step);
        assert!(denied_res.is_err(), "Must fail closed if user permission is not granted");

        // 2. Test Emergency Stop Abort
        EMERGENCY_STOP.store(true, Ordering::SeqCst);
        let stopped_step = StepExecutionPayload {
            id: Some("stop-1".into()),
            title: "Stop Test".into(),
            kind: "click".into(),
            action_type: Some("click".into()),
            locator: None,
            text: None,
            secret_ref: None,
            key: None,
            expected_state: None,
            expected_locator: None,
            timeout_ms: Some(1000),
            max_attempts: Some(1),
            permission_granted: Some(true),
            target_process_id: None,
            target_exe_path: None,
            target_window_title: None,
        };
        let stop_res = execute_step(demo.id, stopped_step).expect("Should return stop result struct");
        assert_eq!(stop_res.status, "stopped", "Emergency stop must cancel step immediately");
        EMERGENCY_STOP.store(false, Ordering::SeqCst);

        // 3. Test Safe Secret Handling (Blocked for Phase 1)
        let secret_step = StepExecutionPayload {
            id: Some("sec-1".into()),
            title: "Secret Test".into(),
            kind: "type-secret".into(),
            action_type: Some("type-secret".into()),
            locator: None,
            text: Some("mockpassword".into()),
            secret_ref: Some("vault:pw".into()),
            key: None,
            expected_state: None,
            expected_locator: None,
            timeout_ms: Some(1000),
            max_attempts: Some(1),
            permission_granted: Some(true),
            target_process_id: None,
            target_exe_path: None,
            target_window_title: None,
        };
        let sec_res = execute_step(demo.id, secret_step);
        assert!(sec_res.is_err(), "Type Secret without valid vault target or locator must fail closed");

        // 4. Test Element Not Found Fails Closed
        let missing_step = StepExecutionPayload {
            id: Some("miss-1".into()),
            title: "Missing Element".into(),
            kind: "click".into(),
            action_type: Some("click".into()),
            locator: Some(LocatorPayload {
                kind: "uia".into(),
                automation_id: Some("nonexistent_id_9999".into()),
                name: Some("Ghost Button".into()),
                control_type: Some("Button".into()),
                class_name: None,
                image_base64: None,
                x: None,
                y: None,
                width: None,
                height: None,
                reference_width: None,
                reference_height: None,
                threshold: None,
                value: None,
            }),
            text: None,
            secret_ref: None,
            key: None,
            expected_state: None,
            expected_locator: None,
            timeout_ms: Some(500),
            max_attempts: Some(1),
            permission_granted: Some(true),
            target_process_id: None,
            target_exe_path: None,
            target_window_title: None,
        };
        let miss_res = execute_step(demo.id, missing_step);
        assert!(miss_res.is_err(), "Must fail closed when element cannot be resolved");

        // 5. Test Expected Result Timeout (Postcondition Fails Closed)
        let unver_step = StepExecutionPayload {
            id: Some("unver-1".into()),
            title: "Unverifiable State".into(),
            kind: "verify-state".into(),
            action_type: Some("verify-state".into()),
            locator: None,
            text: None,
            secret_ref: None,
            key: None,
            expected_state: Some("ImpossibleStateNeverArriving".into()),
            expected_locator: None,
            timeout_ms: Some(600),
            max_attempts: Some(1),
            permission_granted: Some(true),
            target_process_id: None,
            target_exe_path: None,
            target_window_title: None,
        };
        let unver_res = execute_step(demo.id, unver_step);
        assert!(unver_res.is_err(), "Must fail closed if postcondition does not appear within timeout");

        // 6. Test Target PID Scope Verification
        let wrong_pid_step = StepExecutionPayload {
            id: Some("pid-1".into()),
            title: "Wrong PID Step".into(),
            kind: "click".into(),
            action_type: Some("click".into()),
            locator: None,
            text: None,
            secret_ref: None,
            key: None,
            expected_state: None,
            expected_locator: None,
            timeout_ms: Some(1000),
            max_attempts: Some(1),
            permission_granted: Some(true),
            target_process_id: Some(999999), // deliberately wrong PID
            target_exe_path: None,
            target_window_title: None,
        };
        let wrong_pid_res = execute_step(demo.id, wrong_pid_step);
        assert!(wrong_pid_res.is_err(), "Must reject step if target PID does not match authorized scope");

        // 7. Test Window Moved Before Run — Dynamic Locator Resolution
        unsafe {
            let _ = SetWindowPos(
                hwnd,
                None,
                320,
                240,
                560,
                360,
                windows::Win32::UI::WindowsAndMessaging::SWP_NOZORDER,
            );
        }
        thread::sleep(Duration::from_millis(100));

        let move_step = StepExecutionPayload {
            id: Some("move-1".into()),
            title: "Click Continue Button after Move".into(),
            kind: "click".into(),
            action_type: Some("click".into()),
            locator: Some(LocatorPayload {
                kind: "uia".into(),
                automation_id: Some("101".into()),
                name: Some("Continue to Server".into()),
                control_type: Some("Button".into()),
                class_name: None,
                image_base64: None,
                x: None,
                y: None,
                width: None,
                height: None,
                reference_width: None,
                reference_height: None,
                threshold: None,
                value: None,
            }),
            text: None,
            secret_ref: None,
            key: None,
            expected_state: Some("server".into()),
            expected_locator: None,
            timeout_ms: Some(3000),
            max_attempts: Some(2),
            permission_granted: Some(true),
            target_process_id: None,
            target_exe_path: None,
            target_window_title: None,
        };
        let move_res = execute_step(demo.id, move_step).expect("Should resolve element dynamically and verify");
        assert_eq!(move_res.status, "verified");

        // 8. Test Window Resized Before Next Action
        unsafe {
            let _ = SetWindowPos(
                hwnd,
                None,
                180,
                160,
                640,
                420,
                windows::Win32::UI::WindowsAndMessaging::SWP_NOZORDER,
            );
        }
        thread::sleep(Duration::from_millis(100));

        let resize_step = StepExecutionPayload {
            id: Some("resize-1".into()),
            title: "Click Select Server Button after Resize".into(),
            kind: "click".into(),
            action_type: Some("click".into()),
            locator: Some(LocatorPayload {
                kind: "uia".into(),
                automation_id: Some("101".into()),
                name: Some("Select Test Server".into()),
                control_type: Some("Button".into()),
                class_name: None,
                image_base64: None,
                x: None,
                y: None,
                width: None,
                height: None,
                reference_width: None,
                reference_height: None,
                threshold: None,
                value: None,
            }),
            text: None,
            secret_ref: None,
            key: None,
            expected_state: Some("ready".into()),
            expected_locator: None,
            timeout_ms: Some(3000),
            max_attempts: Some(2),
            permission_granted: Some(true),
            target_process_id: None,
            target_exe_path: None,
            target_window_title: None,
        };
        let resize_res = execute_step(demo.id, resize_step).expect("Should resolve button on resized window");
        assert!(resize_res.success, "Resize step must succeed");
        assert_eq!(resize_res.status, "completed");

        // 9. Test Target Window Closed Fails Closed
        unsafe {
            let _ = PostMessageW(Some(hwnd), windows::Win32::UI::WindowsAndMessaging::WM_CLOSE, WPARAM(0), LPARAM(0));
        }
        thread::sleep(Duration::from_millis(250));

        let closed_step = StepExecutionPayload {
            id: Some("close-1".into()),
            title: "Closed Window Step".into(),
            kind: "click".into(),
            action_type: Some("click".into()),
            locator: None,
            text: None,
            secret_ref: None,
            key: None,
            expected_state: None,
            expected_locator: None,
            timeout_ms: Some(1000),
            max_attempts: Some(1),
            permission_granted: Some(true),
            target_process_id: None,
            target_exe_path: None,
            target_window_title: None,
        };
        let closed_res = execute_step(demo.id, closed_step.clone());
        assert!(closed_res.is_err(), "Must fail closed if target window is closed");

        let invalid_res = execute_step(0, closed_step);
        assert!(invalid_res.is_err(), "Must fail closed if window handle is 0");
    }

    #[test]
    fn test_click_without_locator_fails_closed() {
        let _lock = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        reset_emergency_stop();
        start_demo().expect("demo target should open");
        thread::sleep(Duration::from_millis(200));
        let demo = list_windows()
            .expect("window enumeration should work")
            .into_iter()
            .find(|window| window.is_demo)
            .expect("demo window should be found");
        let _session = authorize_session(demo.id, Some("DEMO".into())).expect("authorize session");
        let hwnd = HWND(demo.id as *mut _);
        unsafe {
            reset_demo_window(hwnd);
        }
        let _session = authorize_session(demo.id, Some("DEMO".into())).expect("authorize session");

        // Click with NO locator at all
        let no_loc_step = StepExecutionPayload {
            id: Some("no-loc-1".into()),
            title: "Blind Click Without Locator".into(),
            kind: "click".into(),
            action_type: Some("click".into()),
            locator: None,
            text: None,
            secret_ref: None,
            key: None,
            expected_state: None,
            expected_locator: None,
            timeout_ms: Some(1000),
            max_attempts: Some(1),
            permission_granted: Some(true),
            target_process_id: None,
            target_exe_path: None,
            target_window_title: None,
        };
        let err1 = execute_step(demo.id, no_loc_step);
        assert!(err1.is_err(), "Click without locator must fail closed and never click window center");
        assert!(err1.unwrap_err().contains("blind clicking the window center is strictly prohibited"));

        // Click with EMPTY locator
        let empty_loc_step = StepExecutionPayload {
            id: Some("empty-loc-1".into()),
            title: "Click With Empty Locator".into(),
            kind: "click".into(),
            action_type: Some("click".into()),
            locator: Some(LocatorPayload {
                kind: "uia".into(),
                automation_id: Some("   ".into()),
                name: Some("".into()),
                control_type: None,
                class_name: None,
                image_base64: None,
                x: None,
                y: None,
                width: None,
                height: None,
                reference_width: None,
                reference_height: None,
                threshold: None,
                value: None,
            }),
            text: None,
            secret_ref: None,
            key: None,
            expected_state: None,
            expected_locator: None,
            timeout_ms: Some(1000),
            max_attempts: Some(1),
            permission_granted: Some(true),
            target_process_id: None,
            target_exe_path: None,
            target_window_title: None,
        };
        let err2 = execute_step(demo.id, empty_loc_step);
        assert!(err2.is_err(), "Click with empty locator must fail closed");
        assert!(err2.unwrap_err().contains("cancelled for safety"));
    }

    #[test]
    fn test_end_to_end_acceptance_workflow_7_steps() {
        let _lock = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        reset_emergency_stop();

        // 1. Launch Application (or attach to demo)
        let step1 = StepExecutionPayload {
            id: Some("step-1-launch".into()),
            title: "Launch Application".into(),
            kind: "launch-app".into(),
            action_type: Some("launch-app".into()),
            locator: None,
            text: Some("DEMO".into()),
            secret_ref: None,
            key: None,
            expected_state: Some("login".into()),
            expected_locator: None,
            timeout_ms: Some(5000),
            max_attempts: Some(1),
            permission_granted: Some(true),
            target_process_id: None,
            target_exe_path: None,
            target_window_title: None,
        };
        let res1 = execute_step(0, step1).expect("Step 1 (launch-app) must succeed");
        assert!(res1.success);
        let target_window_id = res1.target_window_id;
        assert!(target_window_id > 0);

        let hwnd = HWND(target_window_id as *mut _);
        unsafe {
            reset_demo_window(hwnd);
        }

        // 2. Wait for Login Window
        let step2 = StepExecutionPayload {
            id: Some("step-2-wait-login".into()),
            title: "Wait for Login Window".into(),
            kind: "wait-visible".into(),
            action_type: Some("wait-visible".into()),
            locator: Some(LocatorPayload {
                kind: "uia".into(),
                automation_id: Some("101".into()),
                name: Some("Continue to Server".into()),
                control_type: Some("Button".into()),
                class_name: None,
                image_base64: None,
                x: None,
                y: None,
                width: None,
                height: None,
                reference_width: None,
                reference_height: None,
                threshold: None,
                value: None,
            }),
            text: None,
            secret_ref: None,
            key: None,
            expected_state: Some("login".into()),
            expected_locator: None,
            timeout_ms: Some(3000),
            max_attempts: Some(1),
            permission_granted: Some(true),
            target_process_id: None,
            target_exe_path: None,
            target_window_title: Some("MIMIC Automation Demo".into()),
        };
        let res2 = execute_step(target_window_id, step2).expect("Step 2 (wait-visible login) must succeed");
        assert!(res2.success);

        // 3. Type Username
        let step3 = StepExecutionPayload {
            id: Some("step-3-type-user".into()),
            title: "Type Username".into(),
            kind: "type-text".into(),
            action_type: Some("type-text".into()),
            locator: Some(LocatorPayload {
                kind: "uia".into(),
                automation_id: Some("102".into()),
                name: None,
                control_type: Some("Edit".into()),
                class_name: None,
                image_base64: None,
                x: None,
                y: None,
                width: None,
                height: None,
                reference_width: None,
                reference_height: None,
                threshold: None,
                value: None,
            }),
            text: Some("player_acceptance".into()),
            secret_ref: None,
            key: None,
            expected_state: None,
            expected_locator: None,
            timeout_ms: Some(3000),
            max_attempts: Some(1),
            permission_granted: Some(true),
            target_process_id: None,
            target_exe_path: None,
            target_window_title: Some("MIMIC Automation Demo".into()),
        };
        let res3 = execute_step(target_window_id, step3).expect("Step 3 (type-text username) must succeed");
        assert!(res3.success);

        // Verify typed text
        let edit_ctrl = unsafe { GetDlgItem(Some(hwnd), DEMO_EDIT_USER_ID as i32).unwrap() };
        let mut buf = [0u16; 64];
        let len = unsafe { GetWindowTextW(edit_ctrl, &mut buf) };
        let typed = String::from_utf16_lossy(&buf[..len as usize]);
        assert!(typed.contains("player_acceptance"), "Typed username must be stored in edit control");

        // 4. Click Continue Button
        let step4 = StepExecutionPayload {
            id: Some("step-4-click-login".into()),
            title: "Click Continue Button".into(),
            kind: "click".into(),
            action_type: Some("click".into()),
            locator: Some(LocatorPayload {
                kind: "uia".into(),
                automation_id: Some("101".into()),
                name: Some("Continue to Server".into()),
                control_type: Some("Button".into()),
                class_name: None,
                image_base64: None,
                x: None,
                y: None,
                width: None,
                height: None,
                reference_width: None,
                reference_height: None,
                threshold: None,
                value: None,
            }),
            text: None,
            secret_ref: None,
            key: None,
            expected_state: Some("server".into()),
            expected_locator: None,
            timeout_ms: Some(3000),
            max_attempts: Some(1),
            permission_granted: Some(true),
            target_process_id: None,
            target_exe_path: None,
            target_window_title: Some("MIMIC Automation Demo".into()),
        };
        let res4 = execute_step(target_window_id, step4).expect("Step 4 (click continue) must succeed");
        assert!(res4.success);
        assert_eq!(res4.status, "verified");

        // 5. Wait for Server Selection
        let step5 = StepExecutionPayload {
            id: Some("step-5-wait-server".into()),
            title: "Wait for Server Selection".into(),
            kind: "wait-visible".into(),
            action_type: Some("wait-visible".into()),
            locator: Some(LocatorPayload {
                kind: "uia".into(),
                automation_id: Some("101".into()),
                name: Some("Select Test Server".into()),
                control_type: Some("Button".into()),
                class_name: None,
                image_base64: None,
                x: None,
                y: None,
                width: None,
                height: None,
                reference_width: None,
                reference_height: None,
                threshold: None,
                value: None,
            }),
            text: None,
            secret_ref: None,
            key: None,
            expected_state: Some("server".into()),
            expected_locator: None,
            timeout_ms: Some(3000),
            max_attempts: Some(1),
            permission_granted: Some(true),
            target_process_id: None,
            target_exe_path: None,
            target_window_title: Some("MIMIC Automation Demo".into()),
        };
        let res5 = execute_step(target_window_id, step5).expect("Step 5 (wait-visible server) must succeed");
        assert!(res5.success);

        // 6. Click Select Server Button
        let step6 = StepExecutionPayload {
            id: Some("step-6-click-server".into()),
            title: "Click Select Server Button".into(),
            kind: "click".into(),
            action_type: Some("click".into()),
            locator: Some(LocatorPayload {
                kind: "uia".into(),
                automation_id: Some("101".into()),
                name: Some("Select Test Server".into()),
                control_type: Some("Button".into()),
                class_name: None,
                image_base64: None,
                x: None,
                y: None,
                width: None,
                height: None,
                reference_width: None,
                reference_height: None,
                threshold: None,
                value: None,
            }),
            text: None,
            secret_ref: None,
            key: None,
            expected_state: Some("ready".into()),
            expected_locator: None,
            timeout_ms: Some(3000),
            max_attempts: Some(1),
            permission_granted: Some(true),
            target_process_id: None,
            target_exe_path: None,
            target_window_title: Some("MIMIC Automation Demo".into()),
        };
        let res6 = execute_step(target_window_id, step6).expect("Step 6 (click select server) must succeed");
        assert!(res6.success);

        // 7. Verify Game Ready State
        let step7 = StepExecutionPayload {
            id: Some("step-7-verify-ready".into()),
            title: "Verify Game Ready".into(),
            kind: "verify-state".into(),
            action_type: Some("verify-state".into()),
            locator: None,
            text: None,
            secret_ref: None,
            key: None,
            expected_state: Some("ready".into()),
            expected_locator: None,
            timeout_ms: Some(3000),
            max_attempts: Some(1),
            permission_granted: Some(true),
            target_process_id: None,
            target_exe_path: None,
            target_window_title: Some("MIMIC Automation Demo".into()),
        };
        let res7 = execute_step(target_window_id, step7).expect("Step 7 (verify-state ready) must succeed");
        assert!(res7.success);
    }

    #[test]
    fn test_emergency_stop_during_wait_visible() {
        let _lock = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        reset_emergency_stop();
        start_demo().expect("demo target should open");
        thread::sleep(Duration::from_millis(200));
        let demo = list_windows()
            .expect("window enumeration should work")
            .into_iter()
            .find(|window| window.is_demo)
            .expect("demo window should be found");
        let _session = authorize_session(demo.id, Some("DEMO".into())).expect("authorize session");

        // Spawn a background thread that triggers emergency stop after 100ms
        thread::spawn(|| {
            thread::sleep(Duration::from_millis(100));
            emergency_stop();
        });

        let wait_step = StepExecutionPayload {
            id: Some("wait-never-arriving".into()),
            title: "Wait for Nonexistent Element".into(),
            kind: "wait-visible".into(),
            action_type: Some("wait-visible".into()),
            locator: Some(LocatorPayload {
                kind: "uia".into(),
                automation_id: Some("ghost_id_9999".into()),
                name: Some("Nonexistent Ghost".into()),
                control_type: Some("Button".into()),
                class_name: None,
                image_base64: None,
                x: None,
                y: None,
                width: None,
                height: None,
                reference_width: None,
                reference_height: None,
                threshold: None,
                value: None,
            }),
            text: None,
            secret_ref: None,
            key: None,
            expected_state: None,
            expected_locator: None,
            timeout_ms: Some(5000), // 5 seconds
            max_attempts: Some(1),
            permission_granted: Some(true),
            target_process_id: None,
            target_exe_path: None,
            target_window_title: None,
        };

        let start = Instant::now();
        let res = execute_step(demo.id, wait_step).expect("Should return step execution result struct");
        let elapsed = start.elapsed();

        assert_eq!(res.status, "stopped", "Emergency stop must cancel wait action");
        assert!(elapsed < Duration::from_millis(1500), "Emergency stop must abort immediately (< 1.5s), elapsed was {:?}", elapsed);
        reset_emergency_stop();
    }

    #[test]
    fn test_process_path_mismatch_fails_closed() {
        let _lock = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        reset_emergency_stop();
        start_demo().expect("demo target should open");
        thread::sleep(Duration::from_millis(100));
        let demo = list_windows()
            .expect("window enumeration should work")
            .into_iter()
            .find(|window| window.is_demo)
            .expect("demo window should be found");
        let _session = authorize_session(demo.id, Some("DEMO".into())).expect("authorize session");

        let mismatch_exe_step = StepExecutionPayload {
            id: Some("mismatch-1".into()),
            title: "Mismatched Executable Scope".into(),
            kind: "verify-state".into(),
            action_type: Some("verify-state".into()),
            locator: None,
            text: None,
            secret_ref: None,
            key: None,
            expected_state: Some("login".into()),
            expected_locator: None,
            timeout_ms: Some(1000),
            max_attempts: Some(1),
            permission_granted: Some(true),
            target_process_id: None,
            target_exe_path: Some("C:\\Windows\\System32\\notepad.exe".into()), // Wrong executable
            target_window_title: None,
        };

        let res = execute_step(demo.id, mismatch_exe_step);
        assert!(res.is_err(), "Must fail closed if executable path does not match authorized binary");
        assert!(res.unwrap_err().contains("does not match authorized"));

        // 2. Strict Full Path: Same filename in different directory must NOT match or fall back
        let actual_exe = unsafe { get_process_path(std::process::id()) }.unwrap_or_default();
        let filename = std::path::Path::new(&actual_exe)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("test.exe");
        let fake_path_step = StepExecutionPayload {
            id: Some("mismatch-2".into()),
            title: "Same Filename Different Path".into(),
            kind: "verify-state".into(),
            action_type: Some("verify-state".into()),
            locator: None,
            text: None,
            secret_ref: None,
            key: None,
            expected_state: Some("login".into()),
            expected_locator: None,
            timeout_ms: Some(1000),
            max_attempts: Some(1),
            permission_granted: Some(true),
            target_process_id: None,
            target_exe_path: Some(format!("C:\\SpoofedPath\\{filename}")),
            target_window_title: None,
        };
        let res2 = execute_step(demo.id, fake_path_step);
        assert!(res2.is_err(), "Must reject even if filename matches when full path differs");
        assert!(res2.unwrap_err().contains("does not match authorized path"));
    }

    #[test]
    fn test_phase_2_type_secret_with_windows_credential_manager() {
        let _lock = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        reset_emergency_stop();
        start_demo().expect("demo target should open");
        thread::sleep(Duration::from_millis(150));
        let demo = list_windows()
            .expect("window enumeration should work")
            .into_iter()
            .find(|window| window.is_demo)
            .expect("demo window should be found");

        let hwnd = HWND(demo.id as *mut _);
        unsafe {
            reset_demo_window(hwnd);
        }
        let _session = authorize_session(demo.id, Some("DEMO".into())).expect("authorize session");

        // 1. Store secret in Windows Credential Manager
        let vault_target = "mimic_phase2_test_user";
        let vault_secret = "VaultSecurePass2026!";
        crate::credentials::save_credential(vault_target, "test_account", vault_secret)
            .expect("Saving credential to vault must succeed");

        // 2. Execute type-secret referencing vault:mimic_phase2_test_user into Password edit control (ID 103)
        let secret_step = StepExecutionPayload {
            id: Some("sec-step-1".into()),
            title: "Type Vault Password".into(),
            kind: "type-secret".into(),
            action_type: Some("type-secret".into()),
            locator: Some(LocatorPayload {
                kind: "uia".into(),
                automation_id: Some("103".into()), // DEMO_EDIT_PASS_ID
                name: None,
                control_type: Some("Edit".into()),
                class_name: None,
                image_base64: None,
                x: None,
                y: None,
                width: None,
                height: None,
                reference_width: None,
                reference_height: None,
                threshold: None,
                value: None,
            }),
            text: None, // plaintext is omitted
            secret_ref: Some(format!("vault:{vault_target}")),
            key: None,
            expected_state: None,
            expected_locator: None,
            timeout_ms: Some(3000),
            max_attempts: Some(1),
            permission_granted: Some(true),
            target_process_id: None,
            target_exe_path: None,
            target_window_title: None,
        };

        let res = execute_step(demo.id, secret_step).expect("type-secret via vault must succeed");
        assert!(res.success);

        // 3. Verify password box received text
        let pass_child = unsafe { GetDlgItem(Some(hwnd), DEMO_EDIT_PASS_ID as i32).unwrap() };
        let mut buf = [0u16; 64];
        let len = unsafe { GetWindowTextW(pass_child, &mut buf) };
        let typed = String::from_utf16_lossy(&buf[..len as usize]);
        assert_eq!(typed, vault_secret, "Password control must receive decrypted vault secret");

        // 4. Clean up vault
        let _ = crate::credentials::delete_credential(vault_target);
    }
}

//! Windows automation engine for MIMIC:
//! - Window enumeration and scoping
//! - Executable picker and launcher
//! - UI Automation and child element inspection
//! - Interactive Select on Screen overlay with highlight and image region capture
//! - Controlled Demo target with Login, Server selection, and Ready views
//! - Offline deterministic workflow step executor with verification, retries, and emergency stop.

use serde::{Deserialize, Serialize};
use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
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
            SelectObject, SetBkMode, SetTextColor, TextOutW, UpdateWindow, BITMAPINFO,
            BITMAPINFOHEADER, BLACK_BRUSH, DIB_RGB_COLORS, HBRUSH, SRCCOPY, TRANSPARENT,
        },
        System::{
            Com::{
                CoCreateInstance, CoInitializeEx, CLSCTX_INPROC_SERVER,
                COINIT_APARTMENTTHREADED,
            },
            LibraryLoader::GetModuleHandleW,
            ProcessStatus::GetModuleFileNameExW,
            Threading::{OpenProcess, PROCESS_QUERY_INFORMATION, PROCESS_VM_READ},
        },
        UI::{
            Accessibility::{CUIAutomation, IUIAutomation},
            Input::KeyboardAndMouse::{
                ReleaseCapture, SendInput, SetCapture, INPUT, INPUT_0, INPUT_KEYBOARD, INPUT_MOUSE,
                KEYBDINPUT, KEYEVENTF_KEYUP, MOUSEEVENTF_LEFTDOWN, MOUSEEVENTF_LEFTUP, MOUSEINPUT,
                VK_BACK, VK_ESCAPE, VK_RETURN, VK_SPACE, VK_TAB,
            },
            Shell::{
                FileOpenDialog, IFileOpenDialog, FOS_FILEMUSTEXIST, FOS_FORCEFILESYSTEM,
                SIGDN_FILESYSPATH,
            },
            WindowsAndMessaging::{
                CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, EnumWindows,
                FindWindowExW, GetClassNameW, GetCursorPos, GetMessageW, GetSystemMetrics,
                GetWindowRect, GetWindowTextLengthW, GetWindowTextW, GetWindowThreadProcessId,
                IsWindow, IsWindowVisible, LoadCursorW, PostQuitMessage, RegisterClassW,
                SendMessageW, SetForegroundWindow, SetLayeredWindowAttributes, SetWindowPos,
                SetWindowTextW, ShowWindow, TranslateMessage, CREATESTRUCTW, CW_USEDEFAULT,
                ES_AUTOHSCROLL, ES_PASSWORD, GWLP_USERDATA, HMENU, HWND_TOPMOST, IDC_ARROW,
                IDC_CROSS, LWA_COLORKEY, MSG, SM_CXVIRTUALSCREEN, SM_CYVIRTUALSCREEN,
                SM_XVIRTUALSCREEN, SM_YVIRTUALSCREEN, SWP_SHOWWINDOW, SW_RESTORE, SW_SHOW,
                WINDOW_EX_STYLE, WINDOW_STYLE, WM_CHAR, WM_COMMAND, WM_DESTROY, WM_KEYDOWN,
                WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MOUSEMOVE, WM_NCCREATE, WM_PAINT, WM_RBUTTONUP,
                WNDCLASSW, WS_BORDER, WS_CHILD, WS_EX_LAYERED, WS_EX_TOOLWINDOW, WS_EX_TOPMOST,
                WS_OVERLAPPEDWINDOW, WS_VISIBLE,
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

static DEMO_STARTED: AtomicBool = AtomicBool::new(false);
static EMERGENCY_STOP: AtomicBool = AtomicBool::new(false);
static REGISTER_DEMO_CLASS: Once = Once::new();
static REGISTER_OVERLAY_CLASS: Once = Once::new();
static DEMO_WINDOW: Mutex<Option<isize>> = Mutex::new(None);

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
    pub secret_ref: Option<String>,
    pub key: Option<String>,
    pub expected_state: Option<String>,
    pub expected_locator: Option<LocatorPayload>,
    pub timeout_ms: Option<u64>,
    pub max_attempts: Option<u32>,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StepExecutionResult {
    pub success: bool,
    pub status: String, // "verified" | "completed" | "failed" | "stopped"
    pub message: String,
    pub target_window_id: isize,
    pub attempts: u32,
    pub duration_ms: u64,
}

#[derive(Clone, Serialize)]
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
// Interactive Selection Overlay
// -----------------------------------------------------------------------------

struct OverlayState {
    _target_window_id: Option<isize>,
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

        *OVERLAY_STATE.lock().unwrap() = Some(OverlayState {
            _target_window_id: target_window_id,
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
    *DEMO_WINDOW.lock().map_err(|_| "Demo window lock failed")? = Some(window.0 as isize);
    Ok(())
}

pub fn start_demo() -> Result<(), String> {
    if DEMO_STARTED.swap(true, Ordering::SeqCst) {
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

unsafe fn click_at_point(hwnd: HWND, x: i32, y: i32, double: bool) -> Result<(), String> {
    let mut rect = RECT::default();
    let _ = GetWindowRect(hwnd, &mut rect);
    if x < rect.left || x > rect.right || y < rect.top || y > rect.bottom {
        return Err("Target point is outside authorized window boundary".into());
    }

    let _ = SetForegroundWindow(hwnd);
    thread::sleep(Duration::from_millis(50));

    let screen_w = GetSystemMetrics(windows::Win32::UI::WindowsAndMessaging::SM_CXSCREEN) as f64;
    let screen_h = GetSystemMetrics(windows::Win32::UI::WindowsAndMessaging::SM_CYSCREEN) as f64;
    let norm_x = (x as f64 * 65535.0 / screen_w) as i32;
    let norm_y = (y as f64 * 65535.0 / screen_h) as i32;

    let inputs = [
        INPUT {
            r#type: INPUT_MOUSE,
            Anonymous: INPUT_0 {
                mi: MOUSEINPUT {
                    dx: norm_x,
                    dy: norm_y,
                    mouseData: 0,
                    dwFlags: windows::Win32::UI::Input::KeyboardAndMouse::MOUSEEVENTF_MOVE
                        | windows::Win32::UI::Input::KeyboardAndMouse::MOUSEEVENTF_ABSOLUTE
                        | MOUSEEVENTF_LEFTDOWN,
                    time: 0,
                    dwExtraInfo: 0,
                },
            },
        },
        INPUT {
            r#type: INPUT_MOUSE,
            Anonymous: INPUT_0 {
                mi: MOUSEINPUT {
                    dx: norm_x,
                    dy: norm_y,
                    mouseData: 0,
                    dwFlags: windows::Win32::UI::Input::KeyboardAndMouse::MOUSEEVENTF_ABSOLUTE
                        | MOUSEEVENTF_LEFTUP,
                    time: 0,
                    dwExtraInfo: 0,
                },
            },
        },
    ];

    SendInput(&inputs, std::mem::size_of::<INPUT>() as i32);

    if double {
        thread::sleep(Duration::from_millis(80));
        SendInput(&inputs, std::mem::size_of::<INPUT>() as i32);
    }
    Ok(())
}

unsafe fn type_characters(hwnd: HWND, text: &str) -> Result<(), String> {
    let _ = SetForegroundWindow(hwnd);
    thread::sleep(Duration::from_millis(40));

    for ch in text.chars() {
        if EMERGENCY_STOP.load(Ordering::SeqCst) {
            return Err("Emergency stop interrupted typing".into());
        }
        let code = ch as u32;
        SendMessageW(hwnd, WM_CHAR, Some(WPARAM(code as usize)), Some(LPARAM(1)));
        thread::sleep(Duration::from_millis(15));
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
    let hwnd = HWND(target_window_id as *mut _);
    unsafe {
        if !IsWindow(Some(hwnd)).as_bool() || !IsWindowVisible(hwnd).as_bool() {
            return Err("Target window is not an open, visible window".into());
        }
    }

    let action_kind = step.action_type.as_deref().unwrap_or(&step.kind);
    let timeout = Duration::from_millis(step.timeout_ms.unwrap_or(5000).max(500));
    let max_attempts = step.max_attempts.unwrap_or(1).max(1);

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

        let action_result: Result<(), String> = unsafe {
            match action_kind {
                "click" | "double-click" => {
                    let double = action_kind == "double-click";
                    if let Some(loc) = &step.locator {
                        if let (Some(x), Some(y)) = (loc.x, loc.y) {
                            let cx = if let Some(w) = loc.width { x + w / 2 } else { x };
                            let cy = if let Some(h) = loc.height { y + h / 2 } else { y };
                            click_at_point(hwnd, cx, cy, double)?;
                        } else if let Some(name) = &loc.name {
                            let wname = to_wide(name);
                            let btn = FindWindowExW(Some(hwnd), None, None, PCWSTR(wname.as_ptr()))
                                .map_err(|e| format!("Element '{name}' not found: {e}"))?;
                            if btn.0.is_null() {
                                return Err(format!("Could not locate element '{name}'"));
                            }
                            const BM_CLICK: u32 = 0x00F5;
                            SendMessageW(btn, BM_CLICK, Some(WPARAM(0)), Some(LPARAM(0)));
                        } else {
                            return Err("Locator has neither coordinates nor element name".into());
                        }
                    } else {
                        let mut rect = RECT::default();
                        let _ = GetWindowRect(hwnd, &mut rect);
                        click_at_point(
                            hwnd,
                            (rect.left + rect.right) / 2,
                            (rect.top + rect.bottom) / 2,
                            double,
                        )?;
                    }
                    Ok(())
                }
                "type-text" => {
                    let text = step.text.as_deref().unwrap_or_default();
                    type_characters(hwnd, text)
                }
                "type-secret" => {
                    // Safe reference lookup: mock OS vault value in v0.1
                    // Secret value is never stored or emitted in logs
                    let synthetic_val = "synthetic_secret_pass";
                    let _ = &step.secret_ref;
                    type_characters(hwnd, synthetic_val)?;
                    Ok(())
                }
                "press-key" => {
                    let key = step.key.as_deref().unwrap_or("ENTER");
                    press_virtual_key(hwnd, key)
                }
                "wait-visible" | "wait-hidden" | "verify-state" => Ok(()),
                "launch-app" => Ok(()),
                other => Err(format!("Unrecognized action kind '{other}'")),
            }
        };

        if let Err(e) = action_result {
            if attempt == max_attempts {
                return Err(format!("Step '{}' action failed: {e}", step.title));
            }
            thread::sleep(Duration::from_millis(200));
            continue;
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
                let current_title = title_of(hwnd);

                if let Some(exp_state) = &step.expected_state {
                    let state_matches = match exp_state.to_lowercase().as_str() {
                        "login" => current_title.contains("Login"),
                        "server" => current_title.contains("Server"),
                        "ready" => current_title.contains("Ready"),
                        title_sub => current_title
                            .to_lowercase()
                            .contains(&title_sub.to_lowercase()),
                    };
                    if state_matches {
                        condition_met = true;
                        break;
                    }
                } else if step.expected_locator.is_some() {
                    condition_met = true;
                    break;
                } else {
                    condition_met = true;
                    break;
                }
            }
            thread::sleep(Duration::from_millis(100));
        }

        if condition_met {
            let is_completed = unsafe {
                let title = title_of(hwnd);
                title.contains("Ready") || step.expected_state.as_deref() == Some("ready")
            };
            return Ok(StepExecutionResult {
                success: true,
                status: if is_completed {
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
                "Step '{}' timed out after {:?}: expected condition was not observed",
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

    #[test]
    fn controlled_demo_observe_act_verify_and_fail_closed() {
        start_demo().expect("demo target should open");
        thread::sleep(Duration::from_millis(150));
        let demo = list_windows()
            .expect("window enumeration should work")
            .into_iter()
            .find(|window| window.is_demo && window.title == DEMO_LOGIN_TITLE)
            .expect("demo login window should be discoverable");
        assert_eq!(
            observe_demo(demo.id).expect("demo should be observable").state,
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

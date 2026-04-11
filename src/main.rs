#![windows_subsystem = "windows"]

use std::ffi::c_void;
use std::mem::size_of;
use std::os::windows::ffi::OsStrExt;
use std::time::{Duration, Instant};
use windows::Win32::Foundation::{
    COLORREF, HWND, LPARAM, LRESULT, POINT, RECT, SIZE, WPARAM,
};
use windows::Win32::Graphics::Gdi::{
    BeginPaint, CLIP_DEFAULT_PRECIS, CLEARTYPE_QUALITY, CreateFontW, CreateSolidBrush,
    DEFAULT_CHARSET, DeleteObject, EndPaint, FF_DONTCARE, FW_BOLD, FillRect, GetTextExtentPoint32W,
    HFONT, InvalidateRect, OUT_OUTLINE_PRECIS, PAINTSTRUCT, SelectObject, SetBkMode, SetTextColor,
    TRANSPARENT, TextOutW,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Input::KeyboardAndMouse::{GetAsyncKeyState, GetKeyboardLayout};
use windows::Win32::UI::Shell::{
    NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE, NOTIFYICONDATAW, Shell_NotifyIconW,
};
use windows::Win32::UI::WindowsAndMessaging::{
    AppendMenuW, CREATESTRUCTW, CS_HREDRAW, CS_VREDRAW, CreatePopupMenu, CreateWindowExW,
    DefWindowProcW, DestroyIcon, DestroyMenu, DestroyWindow, DispatchMessageW, GWLP_USERDATA,
    GetClientRect, GetCursorPos, GetForegroundWindow, GetMessageW, GetSystemMetrics,
    GetWindowLongPtrW, GetWindowThreadProcessId, HICON, IDC_ARROW, IDI_APPLICATION, IMAGE_ICON,
    KillTimer, LR_DEFAULTSIZE, LR_LOADFROMFILE, LWA_ALPHA, LoadCursorW, LoadIconW, LoadImageW,
    MF_CHECKED, MF_SEPARATOR, MF_STRING, MF_UNCHECKED, MSG, PostMessageW, PostQuitMessage,
    RegisterClassW, SM_CXSCREEN, SM_CYSCREEN, SW_HIDE, SW_SHOWNOACTIVATE, SWP_NOACTIVATE,
    SWP_NOZORDER, SetForegroundWindow, SetLayeredWindowAttributes, SetTimer, SetWindowLongPtrW,
    SetWindowPos, ShowWindow, TPM_BOTTOMALIGN, TPM_LEFTALIGN, TPM_RIGHTBUTTON, TrackPopupMenu,
    TranslateMessage, WINDOW_EX_STYLE, WINDOW_STYLE, WM_APP, WM_COMMAND, WM_CONTEXTMENU,
    WM_CREATE, WM_DESTROY, WM_NCCREATE, WM_NCDESTROY, WM_NULL, WM_PAINT, WM_RBUTTONUP, WM_TIMER,
    WNDCLASSW, WS_EX_LAYERED, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_POPUP,
};
use windows::core::{PCWSTR, w};

const WINDOW_WIDTH: i32 = 460;
const WINDOW_HEIGHT: i32 = 240;
const POLL_TIMER_ID: usize = 1;
const HIDE_TIMER_ID: usize = 2;
const DEFAULT_POLL_INTERVAL_MS: u32 = 120;
const DEFAULT_DISPLAY_MS: u32 = 1200;
const DEFAULT_TYPING_SESSION_GAP: Duration = Duration::from_millis(5000);

const TRAY_ICON_ID: u32 = 1;
const TRAY_CALLBACK_MSG: u32 = WM_APP + 1;

const MENU_TOGGLE_PAUSE: usize = 1001;
const MENU_SIZE_SMALL: usize = 1002;
const MENU_SIZE_MEDIUM: usize = 1003;
const MENU_SIZE_LARGE: usize = 1004;

const MENU_POLL_80: usize = 1101;
const MENU_POLL_120: usize = 1102;
const MENU_POLL_200: usize = 1103;

const MENU_DISPLAY_600: usize = 1201;
const MENU_DISPLAY_1200: usize = 1202;
const MENU_DISPLAY_2000: usize = 1203;

const MENU_TYPING_GAP_2S: usize = 1301;
const MENU_TYPING_GAP_5S: usize = 1302;
const MENU_TYPING_GAP_8S: usize = 1303;

const MENU_EXIT: usize = 1099;

#[derive(Copy, Clone, Eq, PartialEq)]
enum IndicatorSize {
    Small,
    Medium,
    Large,
}

impl IndicatorSize {
    fn font_height(self) -> i32 {
        match self {
            IndicatorSize::Small => -96,
            IndicatorSize::Medium => -140,
            IndicatorSize::Large => -196,
        }
    }
}

struct AppState {
    last_hkl: isize,
    initialized: bool,
    paused: bool,
    last_typing_press: Option<Instant>,
    poll_interval_ms: u32,
    display_ms: u32,
    typing_session_gap: Duration,
    size: IndicatorSize,
    text_utf16: Vec<u16>,
    font: HFONT,
    tray_icon: HICON,
    owns_tray_icon: bool,
}

impl AppState {
    fn new() -> Self {
        Self {
            last_hkl: 0,
            initialized: false,
            paused: false,
            last_typing_press: None,
            poll_interval_ms: DEFAULT_POLL_INTERVAL_MS,
            display_ms: DEFAULT_DISPLAY_MS,
            typing_session_gap: DEFAULT_TYPING_SESSION_GAP,
            size: IndicatorSize::Medium,
            text_utf16: to_utf16("EN"),
            font: HFONT::default(),
            tray_icon: HICON::default(),
            owns_tray_icon: false,
        }
    }
}

fn to_utf16(text: &str) -> Vec<u16> {
    text.encode_utf16().collect()
}

fn copy_utf16_z(dst: &mut [u16], text: &str) {
    dst.fill(0);
    let src: Vec<u16> = text.encode_utf16().collect();
    let count = src.len().min(dst.len().saturating_sub(1));
    dst[..count].copy_from_slice(&src[..count]);
}

fn layout_to_indicator(hkl: isize) -> String {
    let lang_id = (hkl as usize as u32) & 0xFFFF;
    let primary_lang = lang_id & 0x03FF;
    match primary_lang {
        0x19 => "RU".to_string(),
        0x09 => "EN".to_string(),
        _ => format!("{lang_id:04X}"),
    }
}

fn center_coords() -> (i32, i32) {
    unsafe {
        let screen_w = GetSystemMetrics(SM_CXSCREEN);
        let screen_h = GetSystemMetrics(SM_CYSCREEN);
        ((screen_w - WINDOW_WIDTH) / 2, (screen_h - WINDOW_HEIGHT) / 2)
    }
}

fn get_state_ptr(hwnd: HWND) -> *mut AppState {
    unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut AppState }
}

fn create_indicator_font(size: IndicatorSize) -> HFONT {
    unsafe {
        CreateFontW(
            size.font_height(),
            0,
            0,
            0,
            FW_BOLD.0 as i32,
            0,
            0,
            0,
            DEFAULT_CHARSET,
            OUT_OUTLINE_PRECIS,
            CLIP_DEFAULT_PRECIS,
            CLEARTYPE_QUALITY,
            FF_DONTCARE.0 as u32,
            w!("Segoe UI"),
        )
    }
}

fn rebuild_font(state: &mut AppState) {
    unsafe {
        if !state.font.0.is_null() {
            let _ = DeleteObject(state.font.into());
        }
        state.font = create_indicator_font(state.size);
    }
}

fn show_indicator(hwnd: HWND, state: &mut AppState, hkl: isize) {
    state.last_hkl = hkl;
    state.text_utf16 = to_utf16(&layout_to_indicator(hkl));

    let (x, y) = center_coords();
    unsafe {
        let _ = SetWindowPos(
            hwnd,
            Some(HWND::default()),
            x,
            y,
            WINDOW_WIDTH,
            WINDOW_HEIGHT,
            SWP_NOACTIVATE | SWP_NOZORDER,
        );
        let _ = InvalidateRect(Some(hwnd), None, true);
        let _ = ShowWindow(hwnd, SW_SHOWNOACTIVATE);
        let _ = KillTimer(Some(hwnd), HIDE_TIMER_ID);
        SetTimer(Some(hwnd), HIDE_TIMER_ID, state.display_ms, None);
    }
}

fn current_hkl() -> isize {
    unsafe {
        let fg = GetForegroundWindow();
        if fg.0.is_null() {
            return 0;
        }

        let thread_id = GetWindowThreadProcessId(fg, None);
        if thread_id == 0 {
            return 0;
        }

        GetKeyboardLayout(thread_id).0 as isize
    }
}

fn is_typing_key_down() -> bool {
    for vk in 0x30..=0x39 {
        if unsafe { (GetAsyncKeyState(vk) as u16 & 0x8000) != 0 } {
            return true;
        }
    }
    for vk in 0x41..=0x5A {
        if unsafe { (GetAsyncKeyState(vk) as u16 & 0x8000) != 0 } {
            return true;
        }
    }
    for vk in 0xBA..=0xC0 {
        if unsafe { (GetAsyncKeyState(vk) as u16 & 0x8000) != 0 } {
            return true;
        }
    }
    for vk in 0xDB..=0xE2 {
        if unsafe { (GetAsyncKeyState(vk) as u16 & 0x8000) != 0 } {
            return true;
        }
    }
    unsafe { (GetAsyncKeyState(0x20) as u16 & 0x8000) != 0 }
}

fn load_custom_tray_icon() -> Option<HICON> {
    let exe_path = std::env::current_exe().ok()?;
    let icon_path = exe_path.with_file_name("tray.ico");
    let wide: Vec<u16> = icon_path
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();

    unsafe {
        let handle = LoadImageW(
            None,
            PCWSTR(wide.as_ptr()),
            IMAGE_ICON,
            0,
            0,
            LR_LOADFROMFILE | LR_DEFAULTSIZE,
        )
        .ok()?;
        if handle.0.is_null() {
            None
        } else {
            Some(HICON(handle.0))
        }
    }
}

fn add_tray_icon(hwnd: HWND, state: &mut AppState) {
    unsafe {
        let (icon, owns_icon) = match load_custom_tray_icon() {
            Some(icon) => (icon, true),
            None => (LoadIconW(None, IDI_APPLICATION).unwrap_or_default(), false),
        };
        state.tray_icon = icon;
        state.owns_tray_icon = owns_icon;

        let mut nid = NOTIFYICONDATAW::default();
        nid.cbSize = size_of::<NOTIFYICONDATAW>() as u32;
        nid.hWnd = hwnd;
        nid.uID = TRAY_ICON_ID;
        nid.uFlags = NIF_MESSAGE | NIF_ICON | NIF_TIP;
        nid.uCallbackMessage = TRAY_CALLBACK_MSG;
        nid.hIcon = state.tray_icon;
        copy_utf16_z(&mut nid.szTip, "Индикатор раскладки");
        let _ = Shell_NotifyIconW(NIM_ADD, &nid);
    }
}

fn remove_tray_icon(hwnd: HWND, state: &mut AppState) {
    unsafe {
        let mut nid = NOTIFYICONDATAW::default();
        nid.cbSize = size_of::<NOTIFYICONDATAW>() as u32;
        nid.hWnd = hwnd;
        nid.uID = TRAY_ICON_ID;
        let _ = Shell_NotifyIconW(NIM_DELETE, &nid);

        if state.owns_tray_icon && !state.tray_icon.0.is_null() {
            let _ = DestroyIcon(state.tray_icon);
        }
        state.tray_icon = HICON::default();
        state.owns_tray_icon = false;
    }
}

fn menu_flags_for_size(current: IndicatorSize, candidate: IndicatorSize) -> windows::Win32::UI::WindowsAndMessaging::MENU_ITEM_FLAGS {
    if current == candidate {
        MF_STRING | MF_CHECKED
    } else {
        MF_STRING | MF_UNCHECKED
    }
}

fn menu_flags_for_u32(
    current: u32,
    candidate: u32,
) -> windows::Win32::UI::WindowsAndMessaging::MENU_ITEM_FLAGS {
    if current == candidate {
        MF_STRING | MF_CHECKED
    } else {
        MF_STRING | MF_UNCHECKED
    }
}

fn menu_flags_for_duration(
    current: Duration,
    candidate: Duration,
) -> windows::Win32::UI::WindowsAndMessaging::MENU_ITEM_FLAGS {
    if current == candidate {
        MF_STRING | MF_CHECKED
    } else {
        MF_STRING | MF_UNCHECKED
    }
}

fn show_tray_menu(hwnd: HWND, state: &AppState) {
    unsafe {
        let menu = match CreatePopupMenu() {
            Ok(menu) => menu,
            Err(_) => return,
        };

        let pause_text = if state.paused {
            w!("Продолжить")
        } else {
            w!("Пауза")
        };
        let _ = AppendMenuW(menu, MF_STRING, MENU_TOGGLE_PAUSE, pause_text);
        let _ = AppendMenuW(menu, MF_SEPARATOR, 0, PCWSTR::null());
        let _ = AppendMenuW(
            menu,
            menu_flags_for_size(state.size, IndicatorSize::Small),
            MENU_SIZE_SMALL,
            w!("Размер: маленький"),
        );
        let _ = AppendMenuW(
            menu,
            menu_flags_for_size(state.size, IndicatorSize::Medium),
            MENU_SIZE_MEDIUM,
            w!("Размер: средний"),
        );
        let _ = AppendMenuW(
            menu,
            menu_flags_for_size(state.size, IndicatorSize::Large),
            MENU_SIZE_LARGE,
            w!("Размер: большой"),
        );
        let _ = AppendMenuW(menu, MF_SEPARATOR, 0, PCWSTR::null());
        let _ = AppendMenuW(
            menu,
            menu_flags_for_u32(state.poll_interval_ms, 80),
            MENU_POLL_80,
            w!("Интервал опроса: 80 мс"),
        );
        let _ = AppendMenuW(
            menu,
            menu_flags_for_u32(state.poll_interval_ms, 120),
            MENU_POLL_120,
            w!("Интервал опроса: 120 мс"),
        );
        let _ = AppendMenuW(
            menu,
            menu_flags_for_u32(state.poll_interval_ms, 200),
            MENU_POLL_200,
            w!("Интервал опроса: 200 мс"),
        );
        let _ = AppendMenuW(menu, MF_SEPARATOR, 0, PCWSTR::null());
        let _ = AppendMenuW(
            menu,
            menu_flags_for_u32(state.display_ms, 600),
            MENU_DISPLAY_600,
            w!("Время показа: 600 мс"),
        );
        let _ = AppendMenuW(
            menu,
            menu_flags_for_u32(state.display_ms, 1200),
            MENU_DISPLAY_1200,
            w!("Время показа: 1200 мс"),
        );
        let _ = AppendMenuW(
            menu,
            menu_flags_for_u32(state.display_ms, 2000),
            MENU_DISPLAY_2000,
            w!("Время показа: 2000 мс"),
        );
        let _ = AppendMenuW(menu, MF_SEPARATOR, 0, PCWSTR::null());
        let _ = AppendMenuW(
            menu,
            menu_flags_for_duration(state.typing_session_gap, Duration::from_secs(2)),
            MENU_TYPING_GAP_2S,
            w!("Пауза печати: 2 с"),
        );
        let _ = AppendMenuW(
            menu,
            menu_flags_for_duration(state.typing_session_gap, Duration::from_secs(5)),
            MENU_TYPING_GAP_5S,
            w!("Пауза печати: 5 с"),
        );
        let _ = AppendMenuW(
            menu,
            menu_flags_for_duration(state.typing_session_gap, Duration::from_secs(8)),
            MENU_TYPING_GAP_8S,
            w!("Пауза печати: 8 с"),
        );
        let _ = AppendMenuW(menu, MF_SEPARATOR, 0, PCWSTR::null());
        let _ = AppendMenuW(menu, MF_STRING, MENU_EXIT, w!("Выход"));

        let mut cursor = POINT::default();
        let _ = GetCursorPos(&mut cursor);
        let _ = SetForegroundWindow(hwnd);
        let _ = TrackPopupMenu(
            menu,
            TPM_LEFTALIGN | TPM_BOTTOMALIGN | TPM_RIGHTBUTTON,
            cursor.x,
            cursor.y,
            Some(0),
            hwnd,
            None,
        );
        let _ = PostMessageW(Some(hwnd), WM_NULL, WPARAM(0), LPARAM(0));
        let _ = DestroyMenu(menu);
    }
}

extern "system" fn wnd_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    unsafe {
        match msg {
            WM_NCCREATE => {
                let create_struct = lparam.0 as *const CREATESTRUCTW;
                let state_ptr = (*create_struct).lpCreateParams as *mut AppState;
                SetWindowLongPtrW(hwnd, GWLP_USERDATA, state_ptr as isize);
                LRESULT(1)
            }
            WM_CREATE => {
                let state_ptr = get_state_ptr(hwnd);
                if !state_ptr.is_null() {
                    let state = &mut *state_ptr;
                    rebuild_font(state);
                    add_tray_icon(hwnd, state);
                    SetTimer(Some(hwnd), POLL_TIMER_ID, state.poll_interval_ms, None);
                } else {
                    SetTimer(Some(hwnd), POLL_TIMER_ID, DEFAULT_POLL_INTERVAL_MS, None);
                }
                let _ = ShowWindow(hwnd, SW_HIDE);
                LRESULT(0)
            }
            WM_COMMAND => {
                let state_ptr = get_state_ptr(hwnd);
                if state_ptr.is_null() {
                    return LRESULT(0);
                }
                let state = &mut *state_ptr;
                let command_id = wparam.0 & 0xFFFF;

                match command_id {
                    MENU_TOGGLE_PAUSE => {
                        state.paused = !state.paused;
                        state.last_typing_press = None;
                        if state.paused {
                            let _ = KillTimer(Some(hwnd), HIDE_TIMER_ID);
                            let _ = ShowWindow(hwnd, SW_HIDE);
                        } else {
                            let hkl = current_hkl();
                            if hkl != 0 {
                                show_indicator(hwnd, state, hkl);
                            }
                        }
                    }
                    MENU_SIZE_SMALL => {
                        state.size = IndicatorSize::Small;
                        rebuild_font(state);
                        let _ = InvalidateRect(Some(hwnd), None, true);
                    }
                    MENU_SIZE_MEDIUM => {
                        state.size = IndicatorSize::Medium;
                        rebuild_font(state);
                        let _ = InvalidateRect(Some(hwnd), None, true);
                    }
                    MENU_SIZE_LARGE => {
                        state.size = IndicatorSize::Large;
                        rebuild_font(state);
                        let _ = InvalidateRect(Some(hwnd), None, true);
                    }
                    MENU_POLL_80 => {
                        state.poll_interval_ms = 80;
                        let _ = KillTimer(Some(hwnd), POLL_TIMER_ID);
                        SetTimer(Some(hwnd), POLL_TIMER_ID, state.poll_interval_ms, None);
                    }
                    MENU_POLL_120 => {
                        state.poll_interval_ms = 120;
                        let _ = KillTimer(Some(hwnd), POLL_TIMER_ID);
                        SetTimer(Some(hwnd), POLL_TIMER_ID, state.poll_interval_ms, None);
                    }
                    MENU_POLL_200 => {
                        state.poll_interval_ms = 200;
                        let _ = KillTimer(Some(hwnd), POLL_TIMER_ID);
                        SetTimer(Some(hwnd), POLL_TIMER_ID, state.poll_interval_ms, None);
                    }
                    MENU_DISPLAY_600 => {
                        state.display_ms = 600;
                    }
                    MENU_DISPLAY_1200 => {
                        state.display_ms = 1200;
                    }
                    MENU_DISPLAY_2000 => {
                        state.display_ms = 2000;
                    }
                    MENU_TYPING_GAP_2S => {
                        state.typing_session_gap = Duration::from_secs(2);
                        state.last_typing_press = None;
                    }
                    MENU_TYPING_GAP_5S => {
                        state.typing_session_gap = Duration::from_secs(5);
                        state.last_typing_press = None;
                    }
                    MENU_TYPING_GAP_8S => {
                        state.typing_session_gap = Duration::from_secs(8);
                        state.last_typing_press = None;
                    }
                    MENU_EXIT => {
                        let _ = DestroyWindow(hwnd);
                    }
                    _ => {}
                }

                LRESULT(0)
            }
            m if m == TRAY_CALLBACK_MSG => {
                let tray_event = lparam.0 as u32;
                if tray_event == WM_CONTEXTMENU || tray_event == WM_RBUTTONUP {
                    let state_ptr = get_state_ptr(hwnd);
                    if !state_ptr.is_null() {
                        show_tray_menu(hwnd, &*state_ptr);
                    }
                }
                LRESULT(0)
            }
            WM_TIMER => {
                let state_ptr = get_state_ptr(hwnd);
                if state_ptr.is_null() {
                    return LRESULT(0);
                }
                let state = &mut *state_ptr;

                if wparam.0 == POLL_TIMER_ID {
                    if state.paused {
                        return LRESULT(0);
                    }

                    let hkl = current_hkl();
                    if hkl != 0 {
                        if !state.initialized {
                            state.initialized = true;
                            show_indicator(hwnd, state, hkl);
                        } else if hkl != state.last_hkl {
                            show_indicator(hwnd, state, hkl);
                        }
                    }

                    if state.initialized && is_typing_key_down() {
                        let now = Instant::now();
                        let should_show = state
                            .last_typing_press
                            .map(|last| now.duration_since(last) >= state.typing_session_gap)
                            .unwrap_or(true);

                        if should_show {
                            let typing_hkl = if hkl != 0 { hkl } else { state.last_hkl };
                            if typing_hkl != 0 {
                                show_indicator(hwnd, state, typing_hkl);
                            }
                        }
                        state.last_typing_press = Some(now);
                    }
                } else if wparam.0 == HIDE_TIMER_ID {
                    let _ = KillTimer(Some(hwnd), HIDE_TIMER_ID);
                    let _ = ShowWindow(hwnd, SW_HIDE);
                }

                LRESULT(0)
            }
            WM_PAINT => {
                let state_ptr = get_state_ptr(hwnd);
                if state_ptr.is_null() {
                    return DefWindowProcW(hwnd, msg, wparam, lparam);
                }
                let state = &mut *state_ptr;

                let mut ps = PAINTSTRUCT::default();
                let hdc = BeginPaint(hwnd, &mut ps);
                let mut rect = RECT::default();
                let _ = GetClientRect(hwnd, &mut rect);

                let bg = CreateSolidBrush(COLORREF(0x00303030));
                FillRect(hdc, &rect, bg);
                let _ = DeleteObject(bg.into());

                let old = SelectObject(hdc, state.font.into());
                SetBkMode(hdc, TRANSPARENT);
                SetTextColor(hdc, COLORREF(0x00F0F0F0));
                let mut text_size = SIZE::default();
                let _ = GetTextExtentPoint32W(hdc, state.text_utf16.as_slice(), &mut text_size);
                let x = (rect.right - text_size.cx) / 2;
                let y = (rect.bottom - text_size.cy) / 2;
                let _ = TextOutW(hdc, x, y, state.text_utf16.as_slice());

                SelectObject(hdc, old);
                let _ = EndPaint(hwnd, &ps);
                LRESULT(0)
            }
            WM_DESTROY => {
                let state_ptr = get_state_ptr(hwnd);
                if !state_ptr.is_null() {
                    remove_tray_icon(hwnd, &mut *state_ptr);
                }
                let _ = KillTimer(Some(hwnd), POLL_TIMER_ID);
                let _ = KillTimer(Some(hwnd), HIDE_TIMER_ID);
                PostQuitMessage(0);
                LRESULT(0)
            }
            WM_NCDESTROY => {
                let state_ptr = get_state_ptr(hwnd);
                if !state_ptr.is_null() {
                    let state = Box::from_raw(state_ptr);
                    if !state.font.0.is_null() {
                        let _ = DeleteObject(state.font.into());
                    }
                    SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
                }
                DefWindowProcW(hwnd, msg, wparam, lparam)
            }
            _ => DefWindowProcW(hwnd, msg, wparam, lparam),
        }
    }
}

fn main() -> windows::core::Result<()> {
    unsafe {
        let instance = GetModuleHandleW(None)?;
        let class_name = w!("LangSwitchIndicatorWindow");

        let cursor = LoadCursorW(None, IDC_ARROW)?;
        let wc = WNDCLASSW {
            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: Some(wnd_proc),
            hInstance: instance.into(),
            hCursor: cursor,
            hbrBackground: Default::default(),
            lpszClassName: class_name,
            ..Default::default()
        };

        if RegisterClassW(&wc) == 0 {
            return Err(windows::core::Error::from_thread());
        }

        let state_ptr = Box::into_raw(Box::new(AppState::new())) as *const c_void;
        let (x, y) = center_coords();

        let hwnd = match CreateWindowExW(
            WINDOW_EX_STYLE(
                WS_EX_TOPMOST.0 | WS_EX_TOOLWINDOW.0 | WS_EX_LAYERED.0 | WS_EX_NOACTIVATE.0,
            ),
            class_name,
            w!(""),
            WINDOW_STYLE(WS_POPUP.0),
            x,
            y,
            WINDOW_WIDTH,
            WINDOW_HEIGHT,
            None,
            None,
            Some(instance.into()),
            Some(state_ptr),
        ) {
            Ok(hwnd) => hwnd,
            Err(err) => {
                drop(Box::from_raw(state_ptr as *mut AppState));
                return Err(err);
            }
        };

        SetLayeredWindowAttributes(hwnd, COLORREF(0), 235, LWA_ALPHA)?;

        let mut msg = MSG::default();
        while GetMessageW(&mut msg, Some(HWND::default()), 0, 0).as_bool() {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }

    Ok(())
}

const std = @import("std");
const w32 = std.os.windows;

const scale = @import("../scale.zig");

const ATOM = w32.ATOM;
const BOOL = w32.BOOL;
const DWORD = w32.DWORD;
const HBRUSH = w32.HBRUSH;
const HCURSOR = w32.HCURSOR;
const HDC = w32.HDC;
const HICON = w32.HICON;
const HINSTANCE = w32.HINSTANCE;
const HMENU = w32.HMENU;
const HWND = w32.HWND;
const LONG = w32.LONG;
const LONG_PTR = w32.LONG_PTR;
const LPARAM = w32.LPARAM;
const LRESULT = w32.LONG_PTR;
const POINT = extern struct { x: LONG, y: LONG };
const RECT = extern struct { left: LONG, top: LONG, right: LONG, bottom: LONG };
const WPARAM = w32.ULONG_PTR;
const WORD = w32.WORD;
const UINT = w32.UINT;

const BITMAPINFO = extern struct {
    bmiHeader: BITMAPINFOHEADER,
    bmiColors: [1]u32,
};

const BITMAPINFOHEADER = extern struct {
    biSize: DWORD,
    biWidth: i32,
    biHeight: i32,
    biPlanes: WORD,
    biBitCount: WORD,
    biCompression: DWORD,
    biSizeImage: DWORD,
    biXPelsPerMeter: i32,
    biYPelsPerMeter: i32,
    biClrUsed: DWORD,
    biClrImportant: DWORD,
};

const MSG = extern struct {
    hwnd: ?HWND,
    message: UINT,
    wParam: WPARAM,
    lParam: LPARAM,
    time: DWORD,
    pt: POINT,
};

const WNDCLASSEXW = extern struct {
    cbSize: UINT,
    style: UINT,
    lpfnWndProc: WNDPROC,
    cbClsExtra: i32,
    cbWndExtra: i32,
    hInstance: HINSTANCE,
    hIcon: ?HICON,
    hCursor: ?HCURSOR,
    hbrBackground: ?HBRUSH,
    lpszMenuName: ?[*:0]const u16,
    lpszClassName: [*:0]const u16,
    hIconSm: ?HICON,
};
const WNDPROC = *const fn (HWND, UINT, WPARAM, LPARAM) callconv(.winapi) LRESULT;

const BI_RGB: DWORD = 0;
const BLACKNESS: DWORD = 0x00000042;
const CLASS_NAME = std.unicode.utf8ToUtf16LeStringLiteral("vega68WindowClass");
const COLORONCOLOR: i32 = 3;
const CW_USEDEFAULT: i32 = @bitCast(@as(u32, 0x80000000));
const DIB_RGB_COLORS: UINT = 0;
const GWL_STYLE: i32 = -16;
const IDC_ARROW: usize = 32512;
const PM_REMOVE: UINT = 0x0001;
const SIM_W: i32 = 1920;
const SIM_H: i32 = 1080;
const SM_CXSCREEN: i32 = 0;
const SM_CYSCREEN: i32 = 1;
const SRCCOPY: DWORD = 0x00CC0020;
const SW_SHOW: i32 = 5;
const SWP_FRAMECHANGED: UINT = 0x0020;
const SWP_NOZORDER: UINT = 0x0004;
const SWP_SHOWWINDOW: UINT = 0x0040;
const VK_ESCAPE: WPARAM = 0x1B;
const VK_F11: WPARAM = 0x7A;
const VK_F12: WPARAM = 0x7B;
const WINDOW_TITLE = std.unicode.utf8ToUtf16LeStringLiteral("vega68");
const WM_CLOSE: UINT = 0x0010;
const WM_ERASEBKGND: UINT = 0x0014;
const WM_DESTROY: UINT = 0x0002;
const WM_KEYDOWN: UINT = 0x0100;
const WM_QUIT: UINT = 0x0012;
const WS_POPUP: DWORD = 0x80000000;
const WS_OVERLAPPEDWINDOW: DWORD = 0x00CF0000;
const WS_VISIBLE: DWORD = 0x10000000;

extern "kernel32" fn GetModuleHandleW(?[*:0]const u16) callconv(.winapi) ?HINSTANCE;
extern "kernel32" fn Sleep(dwMilliseconds: DWORD) callconv(.winapi) void;
extern "user32" fn RegisterClassExW(*const WNDCLASSEXW) callconv(.winapi) ATOM;
extern "user32" fn CreateWindowExW(DWORD, ?[*:0]const u16, ?[*:0]const u16, DWORD, i32, i32, i32, i32, ?HWND, ?HMENU, HINSTANCE, ?*anyopaque) callconv(.winapi) ?HWND;
extern "user32" fn DefWindowProcW(HWND, UINT, WPARAM, LPARAM) callconv(.winapi) LRESULT;
extern "user32" fn ShowWindow(HWND, i32) callconv(.winapi) BOOL;
extern "user32" fn DestroyWindow(HWND) callconv(.winapi) BOOL;
extern "user32" fn PeekMessageW(*MSG, ?HWND, UINT, UINT, UINT) callconv(.winapi) BOOL;
extern "user32" fn TranslateMessage(*const MSG) callconv(.winapi) BOOL;
extern "user32" fn DispatchMessageW(*const MSG) callconv(.winapi) LRESULT;
extern "user32" fn PostQuitMessage(i32) callconv(.winapi) void;
extern "user32" fn GetClientRect(HWND, *RECT) callconv(.winapi) BOOL;
extern "user32" fn GetWindowRect(HWND, *RECT) callconv(.winapi) BOOL;
extern "user32" fn AdjustWindowRect(*RECT, DWORD, BOOL) callconv(.winapi) BOOL;
extern "user32" fn SetWindowLongPtrW(HWND, i32, LONG_PTR) callconv(.winapi) LONG_PTR;
extern "user32" fn SetWindowPos(HWND, ?HWND, i32, i32, i32, i32, UINT) callconv(.winapi) BOOL;
extern "user32" fn GetSystemMetrics(i32) callconv(.winapi) i32;
extern "user32" fn GetDC(?HWND) callconv(.winapi) ?HDC;
extern "user32" fn ReleaseDC(?HWND, HDC) callconv(.winapi) i32;
extern "user32" fn LoadCursorW(?HINSTANCE, [*:0]align(1) const u16) callconv(.winapi) ?HCURSOR;
extern "gdi32" fn SetStretchBltMode(HDC, i32) callconv(.winapi) i32;
extern "gdi32" fn StretchDIBits(HDC, i32, i32, i32, i32, i32, i32, i32, i32, ?*const anyopaque, *const BITMAPINFO, UINT, DWORD) callconv(.winapi) i32;
extern "gdi32" fn PatBlt(HDC, i32, i32, i32, i32, DWORD) callconv(.winapi) BOOL;

const Mode = enum { windowed, fullscreen, sim_1080 };

const Window = struct {
    hwnd: HWND,
    quit: bool = false,
    mode: Mode = .windowed,
    saved: RECT = .{ .left = 0, .top = 0, .right = 0, .bottom = 0 },
};
var g_window: ?Window = null;

fn applyMode(win: *Window, mode: Mode) void {
    const style: DWORD = if (mode == .windowed) WS_OVERLAPPEDWINDOW | WS_VISIBLE else WS_POPUP | WS_VISIBLE;
    _ = SetWindowLongPtrW(win.hwnd, GWL_STYLE, @intCast(style));

    const r: RECT = switch (mode) {
        .windowed => win.saved,
        .fullscreen => .{ .left = 0, .top = 0, .right = GetSystemMetrics(SM_CXSCREEN), .bottom = GetSystemMetrics(SM_CYSCREEN) },
        .sim_1080 => block: {
            const x = @divTrunc(GetSystemMetrics(SM_CXSCREEN) - SIM_W, 2);
            const y = @divTrunc(GetSystemMetrics(SM_CYSCREEN) - SIM_H, 2);
            break :block .{ .left = x, .top = y, .right = x + SIM_W, .bottom = y + SIM_H };
        },
    };
    _ = SetWindowPos(win.hwnd, null, r.left, r.top, r.right - r.left, r.bottom - r.top, SWP_NOZORDER | SWP_FRAMECHANGED | SWP_SHOWWINDOW);
    win.mode = mode;
}

fn toggleMode(win: *Window, target: Mode) void {
    const next: Mode = if (win.mode == target) .windowed else target;
    if (win.mode == .windowed and next != .windowed) _ = GetWindowRect(win.hwnd, &win.saved);
    applyMode(win, next);
}

fn wndProc(hwnd: HWND, msg: UINT, wparam: WPARAM, lparam: LPARAM) callconv(.winapi) LRESULT {
    switch (msg) {
        WM_CLOSE => {
            _ = DestroyWindow(hwnd);
            return 0;
        },
        WM_DESTROY => {
            if (g_window) |*win| win.quit = true;
            PostQuitMessage(0);
            return 0;
        },
        WM_ERASEBKGND => return 1,
        WM_KEYDOWN => {
            if (g_window) |*win| switch (wparam) {
                VK_F11 => toggleMode(win, .fullscreen),
                VK_F12 => toggleMode(win, .sim_1080),
                VK_ESCAPE => if (win.mode != .windowed) applyMode(win, .windowed),
                else => {},
            };
            return 0;
        },
        else => return DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

pub fn create() !void {
    const hinst = GetModuleHandleW(null) orelse return error.NoModuleHandle;

    var wc: WNDCLASSEXW = .{
        .cbSize = @sizeOf(WNDCLASSEXW),
        .style = 0,
        .lpfnWndProc = &wndProc,
        .cbClsExtra = 0,
        .cbWndExtra = 0,
        .hInstance = hinst,
        .hIcon = null,
        .hCursor = LoadCursorW(null, @ptrFromInt(IDC_ARROW)),
        .hbrBackground = null,
        .lpszMenuName = null,
        .lpszClassName = CLASS_NAME,
        .hIconSm = null,
    };
    if (RegisterClassExW(&wc) == 0) return error.RegisterClassFailed;

    var wr: RECT = .{ .left = 0, .top = 0, .right = 960, .bottom = 720 };
    _ = AdjustWindowRect(&wr, WS_OVERLAPPEDWINDOW, .FALSE);

    const hwnd = CreateWindowExW(
        0,
        CLASS_NAME,
        WINDOW_TITLE,
        WS_OVERLAPPEDWINDOW | WS_VISIBLE,
        CW_USEDEFAULT,
        CW_USEDEFAULT,
        wr.right - wr.left,
        wr.bottom - wr.top,
        null,
        null,
        hinst,
        null,
    ) orelse return error.CreateWindowFailed;

    _ = ShowWindow(hwnd, SW_SHOW);
    g_window = .{ .hwnd = hwnd };
}

pub fn destroy() void {
    if (g_window) |win| {
        _ = DestroyWindow(win.hwnd);
        g_window = null;
    }
}

pub fn present(fb: []const u32, width: u32, height: u32) void {
    const win = g_window orelse return;
    const hdc = GetDC(win.hwnd) orelse return;
    defer _ = ReleaseDC(win.hwnd, hdc);

    var rc: RECT = undefined;
    if (!GetClientRect(win.hwnd, &rc).toBool()) return;
    const cw = rc.right - rc.left;
    const ch = rc.bottom - rc.top;

    _ = PatBlt(hdc, 0, 0, cw, ch, BLACKNESS);

    var bmi = std.mem.zeroes(BITMAPINFO);
    bmi.bmiHeader.biSize = @sizeOf(BITMAPINFOHEADER);
    bmi.bmiHeader.biWidth = @intCast(width);
    bmi.bmiHeader.biHeight = -@as(i32, @intCast(height));
    bmi.bmiHeader.biPlanes = 1;
    bmi.bmiHeader.biBitCount = 32;
    bmi.bmiHeader.biCompression = BI_RGB;

    const dst = scale.fit(cw, ch, @intCast(width), @intCast(height));
    _ = SetStretchBltMode(hdc, COLORONCOLOR); // integer scaling
    _ = StretchDIBits(
        hdc,
        dst.x,
        dst.y,
        dst.w,
        dst.h,
        0,
        0,
        @intCast(width),
        @intCast(height),
        @ptrCast(fb.ptr),
        &bmi,
        DIB_RGB_COLORS,
        SRCCOPY,
    );
}

pub fn shouldQuit() bool {
    var msg: MSG = undefined;
    while (PeekMessageW(&msg, null, 0, 0, PM_REMOVE).toBool()) {
        if (msg.message == WM_QUIT) return true;
        _ = TranslateMessage(&msg);
        _ = DispatchMessageW(&msg);
    }
    if (g_window) |win| return win.quit;
    return true;
}

pub fn sleep(ms: u32) void {
    Sleep(ms);
}

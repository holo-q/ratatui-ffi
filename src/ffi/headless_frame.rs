use crate::*;
use std::ffi::CString;

#[no_mangle]
pub extern "C" fn ratatui_headless_render_frame(
    width: u16,
    height: u16,
    cmds: *const FfiDrawCmd,
    len: usize,
    out_text_utf8: *mut *mut c_char,
) -> bool {
    if cmds.is_null() || out_text_utf8.is_null() {
        return false;
    }
    let area = Rect { x: 0, y: 0, width, height };
    let mut buf = Buffer::empty(area);
    let Some(slice) = slice_checked(cmds, len, "headless_render_frame(slice)") else {
        return false;
    };
    for cmd in slice.iter() { crate::ffi::render::render_cmd_to_buffer(cmd, &mut buf); }
    let mut s = String::new();
    for y in 0..height {
        for x in 0..width { s.push_str(buf[(x, y)].symbol()); }
        if y + 1 < height { s.push('\n'); }
    }
    match CString::new(s) {
        Ok(cstr) => { unsafe { *out_text_utf8 = cstr.into_raw(); } true }
        Err(_) => false,
    }
}

#[no_mangle]
pub extern "C" fn ratatui_headless_render_frame_styles(
    width: u16,
    height: u16,
    cmds: *const FfiDrawCmd,
    len: usize,
    out_text_utf8: *mut *mut c_char,
) -> bool {
    if cmds.is_null() || out_text_utf8.is_null() { return false; }
    let area = Rect { x: 0, y: 0, width, height };
    let mut buf = Buffer::empty(area);
    let Some(slice) = slice_checked(cmds, len, "headless_render_frame_styles(slice)") else { return false; };
    for cmd in slice.iter() { crate::ffi::render::render_cmd_to_buffer(cmd, &mut buf); }
    let mut s = String::new();
    for y in 0..height {
        for x in 0..width {
            let cell = &buf[(x, y)];
            let st = cell.style();
            let fg = st.fg.unwrap_or(Color::Reset);
            let bg = st.bg.unwrap_or(Color::Reset);
            let mods = st.add_modifier | st.sub_modifier;
            let to_hex = |c: Color| -> u8 { match c {
                Color::Black => 0x01, Color::Red => 0x02, Color::Green => 0x03, Color::Yellow => 0x04,
                Color::Blue => 0x05, Color::Magenta => 0x06, Color::Cyan => 0x07, Color::Gray => 0x08,
                Color::DarkGray => 0x09, Color::LightRed => 0x0A, Color::LightGreen => 0x0B, Color::LightYellow => 0x0C,
                Color::LightBlue => 0x0D, Color::LightMagenta => 0x0E, Color::LightCyan => 0x0F, Color::White => 0x10,
                _ => 0x00,
            }};
            s.push_str(&format!("{:02X}{:02X}{:04X}", to_hex(fg), to_hex(bg), mods.bits()));
            if x + 1 < width { s.push(' '); }
        }
        if y + 1 < height { s.push('\n'); }
    }
    match CString::new(s) { Ok(cstr) => { unsafe { *out_text_utf8 = cstr.into_raw(); } true }, Err(_) => false }
}

#[no_mangle]
pub extern "C" fn ratatui_headless_render_frame_styles_ex(
    width: u16,
    height: u16,
    cmds: *const FfiDrawCmd,
    len: usize,
    out_text_utf8: *mut *mut c_char,
) -> bool {
    if cmds.is_null() || out_text_utf8.is_null() { return false; }
    let area = Rect { x: 0, y: 0, width, height };
    let mut buf = Buffer::empty(area);
    let Some(slice) = slice_checked(cmds, len, "headless_render_frame_styles_ex(slice)") else { return false; };
    for cmd in slice.iter() { crate::ffi::render::render_cmd_to_buffer(cmd, &mut buf); }
    let mut s = String::new();
    for y in 0..height {
        for x in 0..width {
            let cell = &buf[(x, y)];
            let st = cell.style();
            let fg = st.fg.unwrap_or(Color::Reset);
            let bg = st.bg.unwrap_or(Color::Reset);
            let mods = st.add_modifier | st.sub_modifier;
            s.push_str(&format!("{:08X}{:08X}{:04X}", color_to_u32(fg), color_to_u32(bg), mods.bits()));
            if x + 1 < width { s.push(' '); }
        }
        if y + 1 < height { s.push('\n'); }
    }
    match CString::new(s) { Ok(cstr) => { unsafe { *out_text_utf8 = cstr.into_raw(); } true }, Err(_) => false }
}

#[no_mangle]
pub extern "C" fn ratatui_headless_render_frame_cells(
    width: u16,
    height: u16,
    cmds: *const FfiDrawCmd,
    len: usize,
    out_cells: *mut FfiCellInfo,
    cap: usize,
) -> usize {
    if cmds.is_null() || out_cells.is_null() || cap == 0 { return 0; }
    let area = Rect { x: 0, y: 0, width, height };
    let mut buf = Buffer::empty(area);
    let Some(slice) = slice_checked(cmds, len, "headless_render_frame_cells(slice)") else { return 0; };
    for cmd in slice.iter() { render_cmd_to_buffer(cmd, &mut buf); }
    let total = (width as usize) * (height as usize);
    let n = total.min(cap);
    let mut idx = 0usize;
    for y in 0..height as usize {
        for x in 0..width as usize {
            if idx >= n { break; }
            let cell = &buf[(x as u16, y as u16)];
            let ch = cell.symbol().chars().next().map(|c| c as u32).unwrap_or(0);
            let st = cell.style();
            let fg = color_to_u32(st.fg.unwrap_or(Color::Reset));
            let bg = color_to_u32(st.bg.unwrap_or(Color::Reset));
            let mods = (st.add_modifier | st.sub_modifier).bits();
            unsafe { *out_cells.add(idx) = FfiCellInfo { ch, fg, bg, mods } };
            idx += 1;
        }
    }
    n
}

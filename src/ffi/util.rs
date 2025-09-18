use crate::*;
use ratatui::prelude::{Alignment, Line, Span};
use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::Padding as RtPadding;
use ratatui::widgets::{Block, BorderType as RtBorderType, Borders};

pub fn spans_from_ffi<'a>(spans: *const FfiSpan, len: usize) -> Option<Vec<Span<'static>>> {
    if spans.is_null() {
        return None;
    }
    let slice = unsafe { std::slice::from_raw_parts(spans, len) };
    let mut out: Vec<Span<'static>> = Vec::with_capacity(len);
    for s in slice.iter() {
        if s.text_utf8.is_null() {
            continue;
        }
        let c = unsafe { std::ffi::CStr::from_ptr(s.text_utf8) };
        if let Ok(txt) = c.to_str() {
            out.push(Span::styled(txt.to_string(), style_from_ffi(s.style)));
        }
    }
    Some(out)
}

pub fn build_block_from_adv(
    borders_bits: u8,
    border_type: u32,
    pad_l: u16,
    pad_t: u16,
    pad_r: u16,
    pad_b: u16,
    title_spans: *const FfiSpan,
    title_len: usize,
) -> Block<'static> {
    let mut block = Block::default().borders(borders_from_bits(borders_bits));
    block = block.padding(RtPadding {
        left: pad_l,
        right: pad_r,
        top: pad_t,
        bottom: pad_b,
    });
    block = match border_type {
        1 => block.border_type(RtBorderType::Rounded),
        2 => block.border_type(RtBorderType::Double),
        3 => block.border_type(RtBorderType::Thick),
        4 => block.border_type(RtBorderType::QuadrantInside),
        5 => block.border_type(RtBorderType::QuadrantOutside),
        _ => block.border_type(RtBorderType::Plain),
    };
    if let Some(sp) = spans_from_ffi(title_spans, title_len) {
        block = block.title(Line::from(sp));
    }
    block
}

pub fn apply_block_title_alignment(b: Block<'static>, align_code: u32) -> Block<'static> {
    let align = match align_code {
        1 => Alignment::Center,
        2 => Alignment::Right,
        _ => Alignment::Left,
    };
    b.title_alignment(align)
}

bitflags::bitflags! {
    #[repr(transparent)]
    pub struct FfiBorders: u8 {
        const NONE   = 0;
        const LEFT   = 1<<0;
        const RIGHT  = 1<<1;
        const TOP    = 1<<2;
        const BOTTOM = 1<<3;
    }
}

pub fn borders_from_bits(bits: u8) -> Borders {
    let fb = FfiBorders::from_bits_truncate(bits);
    let mut b = Borders::NONE;
    if fb.contains(FfiBorders::LEFT) {
        b |= Borders::LEFT;
    }
    if fb.contains(FfiBorders::RIGHT) {
        b |= Borders::RIGHT;
    }
    if fb.contains(FfiBorders::TOP) {
        b |= Borders::TOP;
    }
    if fb.contains(FfiBorders::BOTTOM) {
        b |= Borders::BOTTOM;
    }
    b
}

pub fn color_from_u32(c: u32) -> Option<Color> {
    if c == 0 {
        return None;
    }
    if (c & 0x8000_0000) != 0 {
        let r = ((c >> 16) & 0xFF) as u8;
        let g = ((c >> 8) & 0xFF) as u8;
        let b = (c & 0xFF) as u8;
        return Some(Color::Rgb(r, g, b));
    }
    if (c & 0x4000_0000) != 0 {
        let idx = (c & 0xFF) as u8;
        return Some(Color::Indexed(idx));
    }
    match c {
        1 => Some(Color::Black),
        2 => Some(Color::Red),
        3 => Some(Color::Green),
        4 => Some(Color::Yellow),
        5 => Some(Color::Blue),
        6 => Some(Color::Magenta),
        7 => Some(Color::Cyan),
        8 => Some(Color::Gray),
        9 => Some(Color::DarkGray),
        10 => Some(Color::LightRed),
        11 => Some(Color::LightGreen),
        12 => Some(Color::LightYellow),
        13 => Some(Color::LightBlue),
        14 => Some(Color::LightMagenta),
        15 => Some(Color::LightCyan),
        16 => Some(Color::White),
        _ => None,
    }
}

pub fn color_to_u32(c: Color) -> u32 {
    match c {
        Color::Reset => 0,
        Color::Black => 1,
        Color::Red => 2,
        Color::Green => 3,
        Color::Yellow => 4,
        Color::Blue => 5,
        Color::Magenta => 6,
        Color::Cyan => 7,
        Color::Gray => 8,
        Color::DarkGray => 9,
        Color::LightRed => 10,
        Color::LightGreen => 11,
        Color::LightYellow => 12,
        Color::LightBlue => 13,
        Color::LightMagenta => 14,
        Color::LightCyan => 15,
        Color::White => 16,
        Color::Indexed(i) => 0x4000_0000 | (i as u32),
        Color::Rgb(r, g, b) => 0x8000_0000 | ((r as u32) << 16) | ((g as u32) << 8) | (b as u32),
    }
}

pub fn style_from_ffi(s: FfiStyle) -> Style {
    let mut st = Style::default();
    if let Some(fg) = color_from_u32(s.fg) {
        st = st.fg(fg);
    }
    if let Some(bg) = color_from_u32(s.bg) {
        st = st.bg(bg);
    }
    let mods = FfiStyleMods::from_bits_truncate(s.mods);
    if mods.contains(FfiStyleMods::BOLD) {
        st = st.add_modifier(Modifier::BOLD);
    }
    if mods.contains(FfiStyleMods::ITALIC) {
        st = st.add_modifier(Modifier::ITALIC);
    }
    if mods.contains(FfiStyleMods::UNDERLINE) {
        st = st.add_modifier(Modifier::UNDERLINED);
    }
    if mods.contains(FfiStyleMods::DIM) {
        st = st.add_modifier(Modifier::DIM);
    }
    if mods.contains(FfiStyleMods::CROSSED) {
        st = st.add_modifier(Modifier::CROSSED_OUT);
    }
    if mods.contains(FfiStyleMods::REVERSED) {
        st = st.add_modifier(Modifier::REVERSED);
    }
    if mods.contains(FfiStyleMods::RAPIDBLINK) {
        st = st.add_modifier(Modifier::RAPID_BLINK);
    }
    if mods.contains(FfiStyleMods::SLOWBLINK) {
        st = st.add_modifier(Modifier::SLOW_BLINK);
    }
    if mods.contains(FfiStyleMods::HIDDEN) {
        st = st.add_modifier(Modifier::HIDDEN);
    }
    st
}

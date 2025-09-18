// Widget split placeholder: Scrollbar (feature = "scrollbar")
// Move from src/lib.rs (under cfg(feature = "scrollbar")):
// - Types: FfiScrollbarOrient, FfiScrollbar, FfiScrollbarOrientation, FfiScrollDirection (enums under cfg)
// - FFI externs: ratatui_scrollbar_new, ratatui_scrollbar_free, ratatui_scrollbar_configure,
//                ratatui_scrollbar_set_orientation_side
// - Block helpers (macros invoked here):
//   ratatui_block_title_fn!(ratatui_scrollbar_set_block_title, FfiScrollbar)
//   ratatui_block_title_spans_fn!(ratatui_scrollbar_set_block_title_spans, FfiScrollbar)
//   ratatui_block_title_alignment_fn!(ratatui_scrollbar_set_block_title_alignment, FfiScrollbar)
//   ratatui_block_adv_fn!(ratatui_scrollbar_set_block_adv, FfiScrollbar)
// - Draw helpers: ratatui_terminal_draw_scrollbar_in, ratatui_headless_render_scrollbar

// use crate::*; // enable when moving implementations

use crate::{
    ratatui_block_adv_fn, ratatui_block_title_alignment_fn, ratatui_block_title_fn,
    ratatui_block_title_spans_fn, FfiRect, FfiTerminal,
};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::widgets::{
    Block, Scrollbar as RtScrollbar, ScrollbarOrientation as RtScrollbarOrientation,
    ScrollbarState as RtScrollbarState,
};
use std::ffi::{c_char, CString};

// ----- Scrollbar -----

#[cfg(feature = "scrollbar")]
#[cfg_attr(docsrs, doc(cfg(feature = "scrollbar")))]
ratatui_block_title_fn!(ratatui_scrollbar_set_block_title, FfiScrollbar);

#[cfg(feature = "scrollbar")]
#[cfg_attr(docsrs, doc(cfg(feature = "scrollbar")))]
ratatui_block_title_spans_fn!(ratatui_scrollbar_set_block_title_spans, FfiScrollbar);

#[cfg(feature = "scrollbar")]
#[cfg_attr(docsrs, doc(cfg(feature = "scrollbar")))]
ratatui_block_adv_fn!(ratatui_scrollbar_set_block_adv, FfiScrollbar);
ratatui_block_title_alignment_fn!(ratatui_scrollbar_set_block_title_alignment, FfiScrollbar);

#[cfg(feature = "scrollbar")]
#[cfg_attr(docsrs, doc(cfg(feature = "scrollbar")))]
#[repr(u32)]
pub enum FfiScrollbarOrient {
    Vertical = 0,
    Horizontal = 1,
}

#[cfg(feature = "scrollbar")]
#[cfg_attr(docsrs, doc(cfg(feature = "scrollbar")))]
#[repr(C)]
pub struct FfiScrollbar {
    pub orient: u32,
    pub position: u16,
    pub content_len: u16,
    pub viewport_len: u16,
    pub block: Option<Block<'static>>,
    pub side: Option<u32>,
}

#[cfg(feature = "scrollbar")]
#[allow(dead_code)]
#[repr(u32)]
pub enum FfiScrollbarOrientation {
    VerticalRight = 0,
    VerticalLeft = 1,
    HorizontalBottom = 2,
    HorizontalTop = 3,
}

#[cfg(feature = "scrollbar")]
#[allow(dead_code)]
#[repr(u32)]
pub enum FfiScrollDirection {
    Forward = 0,
    Backward = 1,
}

#[no_mangle]
#[cfg(feature = "scrollbar")]
#[cfg_attr(docsrs, doc(cfg(feature = "scrollbar")))]
pub extern "C" fn ratatui_scrollbar_new() -> *mut FfiScrollbar {
    Box::into_raw(Box::new(FfiScrollbar {
        orient: FfiScrollbarOrient::Vertical as u32,
        position: 0,
        content_len: 0,
        viewport_len: 0,
        block: None,
        side: None,
    }))
}

#[no_mangle]
#[cfg(feature = "scrollbar")]
#[cfg_attr(docsrs, doc(cfg(feature = "scrollbar")))]
pub extern "C" fn ratatui_scrollbar_free(s: *mut FfiScrollbar) {
    if s.is_null() {
        return;
    }
    unsafe {
        drop(Box::from_raw(s));
    }
}

#[no_mangle]
#[cfg(feature = "scrollbar")]
#[cfg_attr(docsrs, doc(cfg(feature = "scrollbar")))]
pub extern "C" fn ratatui_scrollbar_configure(
    s: *mut FfiScrollbar,
    orient: u32,
    position: u16,
    content_len: u16,
    viewport_len: u16,
) {
    if s.is_null() {
        return;
    }
    let sb = unsafe { &mut *s };
    sb.orient = orient;
    sb.position = position;
    sb.content_len = content_len;
    sb.viewport_len = viewport_len;
}

#[no_mangle]
#[cfg(feature = "scrollbar")]
#[cfg_attr(docsrs, doc(cfg(feature = "scrollbar")))]
pub extern "C" fn ratatui_scrollbar_set_orientation_side(s: *mut FfiScrollbar, side: u32) {
    // side mapping: 0=VerticalLeft, 1=VerticalRight, 2=HorizontalTop, 3=HorizontalBottom
    if s.is_null() {
        return;
    }
    let sb = unsafe { &mut *s };
    sb.side = Some(side);
}

#[no_mangle]
#[cfg(feature = "scrollbar")]
#[cfg_attr(docsrs, doc(cfg(feature = "scrollbar")))]
pub extern "C" fn ratatui_terminal_draw_scrollbar_in(
    term: *mut FfiTerminal,
    s: *const FfiScrollbar,
    rect: FfiRect,
) -> bool {
    if term.is_null() || s.is_null() {
        return false;
    }
    let t = unsafe { &mut *term };
    let sb = unsafe { &*s };
    let area = Rect {
        x: rect.x,
        y: rect.y,
        width: rect.width,
        height: rect.height,
    };
    let orient = if let Some(side) = sb.side {
        match side {
            0 => RtScrollbarOrientation::VerticalLeft,
            1 => RtScrollbarOrientation::VerticalRight,
            2 => RtScrollbarOrientation::HorizontalTop,
            3 => RtScrollbarOrientation::HorizontalBottom,
            _ => RtScrollbarOrientation::VerticalRight,
        }
    } else if sb.orient == FfiScrollbarOrient::Horizontal as u32 {
        RtScrollbarOrientation::HorizontalTop
    } else {
        RtScrollbarOrientation::VerticalRight
    };
    let mut state = RtScrollbarState::new(sb.content_len as usize)
        .position(sb.position as usize)
        .viewport_content_length(sb.viewport_len as usize);
    let w = RtScrollbar::new(orient);
    let res = t.terminal.draw(|frame| {
        frame.render_stateful_widget(w.clone(), area, &mut state);
    });
    res.is_ok()
}

#[no_mangle]
#[cfg(feature = "scrollbar")]
#[cfg_attr(docsrs, doc(cfg(feature = "scrollbar")))]
pub extern "C" fn ratatui_headless_render_scrollbar(
    width: u16,
    height: u16,
    s: *const FfiScrollbar,
    out_text_utf8: *mut *mut c_char,
) -> bool {
    if s.is_null() || out_text_utf8.is_null() {
        return false;
    }
    let sb = unsafe { &*s };
    let area = Rect {
        x: 0,
        y: 0,
        width,
        height,
    };
    let mut buf = Buffer::empty(area);
    let orient = if let Some(side) = sb.side {
        match side {
            0 => RtScrollbarOrientation::VerticalLeft,
            1 => RtScrollbarOrientation::VerticalRight,
            2 => RtScrollbarOrientation::HorizontalTop,
            3 => RtScrollbarOrientation::HorizontalBottom,
            _ => RtScrollbarOrientation::VerticalRight,
        }
    } else if sb.orient == FfiScrollbarOrient::Horizontal as u32 {
        RtScrollbarOrientation::HorizontalTop
    } else {
        RtScrollbarOrientation::VerticalRight
    };
    let mut state = RtScrollbarState::new(sb.content_len as usize)
        .position(sb.position as usize)
        .viewport_content_length(sb.viewport_len as usize);
    let w = RtScrollbar::new(orient);
    ratatui::widgets::StatefulWidget::render(w, area, &mut buf, &mut state);
    let mut s = String::new();
    for y in 0..height {
        for x in 0..width {
            let cell = &buf[(x, y)];
            s.push_str(cell.symbol());
        }
        if y + 1 < height {
            s.push('\n');
        }
    }
    match CString::new(s) {
        Ok(cstr) => {
            unsafe {
                *out_text_utf8 = cstr.into_raw();
            }
            true
        }
        Err(_) => false,
    }
}

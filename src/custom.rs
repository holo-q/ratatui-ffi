use crate::*;
use ratatui::layout::Rect;

/// Custom, non-upstream extension APIs for high-performance composition.
/// These are intentionally kept in a separate module so they can evolve
/// independently and be proposed upstream selectively.

#[no_mangle]
pub extern "C" fn ratatui_terminal_draw_cells_in(
    term: *mut FfiTerminal,
    cells: *const FfiCellInfo,
    width: u16,
    height: u16,
    rect: FfiRect,
) -> bool {
    guard_bool("ratatui_terminal_draw_cells_in", || {
        if term.is_null() || cells.is_null() {
            return false;
        }
        let t = unsafe { &mut *term };

        // Validate and materialize the input cell slice.
        let expected = (width as usize).saturating_mul(height as usize);
        if expected == 0 {
            return true;
        }
        let Some(slice) = crate::slice_checked(cells, expected, "terminal_draw_cells_in(cells)") else {
            return false;
        };

        let area = Rect {
            x: rect.x,
            y: rect.y,
            width: rect.width,
            height: rect.height,
        };

        let res = t.terminal.draw(|frame| {
            // Clip to frame bounds defensively.
            let frame_area = frame.size();
            if area.x >= frame_area.width || area.y >= frame_area.height {
                return; // nothing visible
            }
            let avail_w = frame_area.width.saturating_sub(area.x);
            let avail_h = frame_area.height.saturating_sub(area.y);
            let w = width.min(avail_w);
            let h = height.min(avail_h);

            let buf = frame.buffer_mut();
            for y in 0..h as usize {
                for x in 0..w as usize {
                    let idx = y * (width as usize) + x;
                    let ci = &slice[idx];
                    let ch = std::char::from_u32(ci.ch).unwrap_or(' ');
                    let style = crate::ffi::util::style_from_ffi(FfiStyle { fg: ci.fg, bg: ci.bg, mods: ci.mods });
                    let bx = area.x as usize + x;
                    let by = area.y as usize + y;
                    // Safe given clipping above
                    let cell = &mut buf[(bx as u16, by as u16)];
                    cell.set_char(ch);
                    cell.set_style(style);
                }
            }
        });
        res.is_ok()
    })
}

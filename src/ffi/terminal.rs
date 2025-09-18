use crate::*;
use crossterm::{
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use std::io::stdout;

#[no_mangle]
pub extern "C" fn ratatui_init_terminal() -> *mut FfiTerminal {
    crate::guard_ptr("ratatui_init_terminal", || {
        let mut out = stdout();
        let want_raw = std::env::var("RATATUI_FFI_NO_RAW").is_err();
        let use_alt = std::env::var("RATATUI_FFI_ALTSCR").is_ok();
        let mut entered_alt = false;
        let mut raw_mode = false;
        if want_raw {
            if enable_raw_mode().is_ok() {
                raw_mode = true;
            }
        }
        if use_alt {
            if execute!(out, EnterAlternateScreen).is_ok() {
                entered_alt = true;
            }
        }
        let backend = CrosstermBackend::new(out);
        match Terminal::new(backend) {
            Ok(mut terminal) => {
                let _ = terminal.hide_cursor();
                let _ = terminal.clear();
                Box::into_raw(Box::new(FfiTerminal {
                    terminal,
                    entered_alt,
                    raw_mode,
                }))
            }
            Err(_) => {
                if raw_mode {
                    let _ = disable_raw_mode();
                }
                std::ptr::null_mut()
            }
        }
    })
}

#[no_mangle]
pub extern "C" fn ratatui_terminal_clear(term: *mut FfiTerminal) {
    crate::guard_void("ratatui_terminal_clear", || {
        if term.is_null() {
            return;
        }
        let t = unsafe { &mut *term };
        let _ = t.terminal.clear();
    })
}

#[no_mangle]
pub extern "C" fn ratatui_terminal_free(term: *mut FfiTerminal) {
    crate::guard_void("ratatui_terminal_free", || {
        if term.is_null() {
            return;
        }
        let mut boxed = unsafe { Box::from_raw(term) };
        let _ = boxed.terminal.show_cursor();
        if boxed.entered_alt {
            let _ = execute!(stdout(), LeaveAlternateScreen);
        }
        if boxed.raw_mode {
            let _ = disable_raw_mode();
        }
    })
}

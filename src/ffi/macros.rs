#[macro_export]
macro_rules! ratatui_block_title_alignment_fn {
    ($fn_name:ident, $ffi_ty:ty) => {
        #[no_mangle]
        pub extern "C" fn $fn_name(ptr: *mut $ffi_ty, align: u32) {
            if ptr.is_null() { return; }
            let obj = unsafe { &mut *ptr };
            let base = obj.block.take().unwrap_or_else(|| ratatui::widgets::Block::default());
            obj.block = Some(crate::apply_block_title_alignment(base, align));
        }
    };
}

#[macro_export]
macro_rules! ratatui_block_adv_fn {
    ($fn_name:ident, $ffi_ty:ty) => {
        #[no_mangle]
        pub extern "C" fn $fn_name(
            ptr: *mut $ffi_ty,
            borders_bits: u8,
            border_type: u32,
            pad_l: u16,
            pad_t: u16,
            pad_r: u16,
            pad_b: u16,
            title_spans: *const crate::FfiSpan,
            title_len: usize,
        ) {
            if ptr.is_null() { return; }
            let obj = unsafe { &mut *ptr };
            obj.block = Some(crate::build_block_from_adv(
                borders_bits,
                border_type,
                pad_l,
                pad_t,
                pad_r,
                pad_b,
                title_spans,
                title_len,
            ));
        }
    };
}

// -------- Const getters --------

// Returns a pointer/len pair to a &'static str constant from ratatui.
#[macro_export]
macro_rules! ratatui_const_str_getter {
    ($fn_name:ident, $path:path) => {
        #[no_mangle]
        pub extern "C" fn $fn_name() -> crate::FfiStr {
            let s: &'static str = $path;
            crate::FfiStr { ptr: s.as_ptr(), len: s.len() }
        }
    };
}

// Returns a Unicode scalar value (u32) for a `char` constant.
#[macro_export]
macro_rules! ratatui_const_char_getter {
    ($fn_name:ident, $path:path) => {
        #[no_mangle]
        pub extern "C" fn $fn_name() -> u32 {
            let ch: char = $path;
            ch as u32
        }
    };
}

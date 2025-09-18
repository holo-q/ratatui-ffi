#[macro_export]
macro_rules! ratatui_block_title_alignment_fn {
    ($fn_name:ident, $ffi_ty:ty) => {
        #[no_mangle]
        pub extern "C" fn $fn_name(ptr: *mut $ffi_ty, align: u32) {
            if ptr.is_null() {
                return;
            }
            let obj = unsafe { &mut *ptr };
            let base = obj
                .block
                .take()
                .unwrap_or_else(|| ratatui::widgets::Block::default());
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
            if ptr.is_null() {
                return;
            }
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
            crate::FfiStr {
                ptr: s.as_ptr(),
                len: s.len(),
            }
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

// Returns a u16 constant as u16
#[macro_export]
macro_rules! ratatui_const_u16_getter {
    ($fn_name:ident, $path:path) => {
        #[no_mangle]
        pub extern "C" fn $fn_name() -> u16 {
            let v: u16 = $path;
            v
        }
    };
}

// Build an FfiStr from a &'static str
#[inline]
pub(crate) fn __ffi_str(s: &'static str) -> crate::FfiStr {
    crate::FfiStr {
        ptr: s.as_ptr(),
        len: s.len(),
    }
}

// Define an FFI struct composed of FfiStr fields.
#[macro_export]
macro_rules! ratatui_define_ffi_str_struct {
    ($ffi_name:ident : $( $field:ident ),+ $(,)? ) => {
        #[repr(C)]
        #[derive(Copy, Clone)]
        pub struct $ffi_name { $( pub $field: crate::FfiStr, )+ }
    };
}

// Block title setter from UTF-8 C string
#[macro_export]
macro_rules! ratatui_block_title_fn {
    ($fn_name:ident, $ffi_ty:ty) => {
        #[no_mangle]
        pub extern "C" fn $fn_name(
            ptr: *mut $ffi_ty,
            title_utf8: *const ::std::os::raw::c_char,
            show_border: bool,
        ) {
            if ptr.is_null() {
                return;
            }
            let obj = unsafe { &mut *ptr };
            let mut block = if show_border {
                ratatui::widgets::Block::default().borders(ratatui::widgets::Borders::ALL)
            } else {
                ratatui::widgets::Block::default()
            };
            if !title_utf8.is_null() {
                let c_str = unsafe { ::std::ffi::CStr::from_ptr(title_utf8) };
                if let Ok(title) = c_str.to_str() {
                    block = block.title(title.to_string());
                }
            }
            obj.block = Some(block);
        }
    };
}

// Block title setter from spans
#[macro_export]
macro_rules! ratatui_block_title_spans_fn {
    ($fn_name:ident, $ffi_ty:ty) => {
        #[no_mangle]
        pub extern "C" fn $fn_name(
            ptr: *mut $ffi_ty,
            title_spans: *const $crate::FfiSpan,
            title_len: usize,
            show_border: bool,
        ) {
            if ptr.is_null() {
                return;
            }
            let obj = unsafe { &mut *ptr };
            let mut block = if show_border {
                ratatui::widgets::Block::default().borders(ratatui::widgets::Borders::ALL)
            } else {
                ratatui::widgets::Block::default()
            };
            if let Some(sp) = $crate::spans_from_ffi(title_spans, title_len) {
                block = block.title(ratatui::text::Line::from(sp));
            }
            obj.block = Some(block);
        }
    };
}

// Generic struct getter builder: maps a source struct's &str fields into an FfiStr struct.
#[macro_export]
macro_rules! ratatui_const_struct_getter {
    ($fn_name:ident, $ffi_name:ident, $src:path, [ $( $field:ident ),+ $(,)? ]) => {
        #[no_mangle]
        pub extern "C" fn $fn_name() -> $crate::$ffi_name {
            let s = $src;
            $crate::$ffi_name { $( $field: $crate::ffi::macros::__ffi_str(s.$field) ),+ }
        }
    };
}

// Define an FFI struct composed of u32 fields (for color palettes as u32 encoded colors)
#[macro_export]
macro_rules! ratatui_define_ffi_u32_struct {
    ($ffi_name:ident : $( $field:ident ),+ $(,)? ) => {
        #[repr(C)]
        #[derive(Copy, Clone)]
        pub struct $ffi_name { $( pub $field: u32, )+ }
    };
}

// Get a Color constant as u32 using crate::color_to_u32
#[macro_export]
macro_rules! ratatui_const_color_u32_getter {
    ($fn_name:ident, $path:path) => {
        #[no_mangle]
        pub extern "C" fn $fn_name() -> u32 {
            let c = $path;
            $crate::color_to_u32(c)
        }
    };
}

// Generic palette struct getter: maps a source struct's Color fields into u32 via color_to_u32
#[macro_export]
macro_rules! ratatui_const_palette_u32_getter {
    ($fn_name:ident, $ffi_name:ident, $src:path, [ $( $field:ident ),+ $(,)? ]) => {
        #[no_mangle]
        pub extern "C" fn $fn_name() -> $crate::$ffi_name {
            let s = $src;
            $crate::$ffi_name { $( $field: $crate::color_to_u32(s.$field) ),+ }
        }
    };
}

// Reserve capacity on a Vec field inside an FFI struct
#[macro_export]
macro_rules! ratatui_reserve_vec_fn {
    ($fn_name:ident, $ffi_ty:ty, $field:ident) => {
        #[no_mangle]
        pub extern "C" fn $fn_name(ptr: *mut $ffi_ty, additional: usize) {
            if ptr.is_null() {
                return;
            }
            unsafe {
                (&mut *ptr).$field.reserve(additional);
            }
        }
    };
}

// Set a Style field from FfiStyle (stores Some(style))
#[macro_export]
macro_rules! ratatui_set_style_fn {
    ($fn_name:ident, $ffi_ty:ty, $field:ident) => {
        #[no_mangle]
        pub extern "C" fn $fn_name(ptr: *mut $ffi_ty, style: $crate::FfiStyle) {
            if ptr.is_null() {
                return;
            }
            unsafe {
                (&mut *ptr).$field = Some($crate::style_from_ffi(style));
            }
        }
    };
}

// Set an Option<usize> field from i32 where <0 => None
#[macro_export]
macro_rules! ratatui_set_selected_i32_fn {
    ($fn_name:ident, $ffi_ty:ty, $field:ident) => {
        #[no_mangle]
        pub extern "C" fn $fn_name(ptr: *mut $ffi_ty, selected: i32) {
            if ptr.is_null() {
                return;
            }
            let v = if selected < 0 {
                None
            } else {
                Some(selected as usize)
            };
            unsafe {
                (&mut *ptr).$field = v;
            }
        }
    };
}

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
pub(crate) fn __ffi_str(s: &'static str) -> crate::FfiStr { crate::FfiStr { ptr: s.as_ptr(), len: s.len() } }

// Define an FFI struct composed of FfiStr fields.
#[macro_export]
macro_rules! ratatui_define_ffi_str_struct {
    ($ffi_name:ident : $( $field:ident ),+ $(,)? ) => {
        #[repr(C)]
        #[derive(Copy, Clone)]
        pub struct $ffi_name { $( pub $field: crate::FfiStr, )+ }
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

// line::Set -> FfiLineSet
#[macro_export]
macro_rules! ratatui_const_line_set_getter {
    ($fn_name:ident, $path:path) => {
        #[no_mangle]
        pub extern "C" fn $fn_name() -> crate::FfiLineSet {
            let s = $path;
            crate::FfiLineSet {
                vertical: $crate::ffi::macros::__ffi_str(s.vertical),
                horizontal: $crate::ffi::macros::__ffi_str(s.horizontal),
                top_right: $crate::ffi::macros::__ffi_str(s.top_right),
                top_left: $crate::ffi::macros::__ffi_str(s.top_left),
                bottom_right: $crate::ffi::macros::__ffi_str(s.bottom_right),
                bottom_left: $crate::ffi::macros::__ffi_str(s.bottom_left),
                vertical_left: $crate::ffi::macros::__ffi_str(s.vertical_left),
                vertical_right: $crate::ffi::macros::__ffi_str(s.vertical_right),
                horizontal_down: $crate::ffi::macros::__ffi_str(s.horizontal_down),
                horizontal_up: $crate::ffi::macros::__ffi_str(s.horizontal_up),
                cross: $crate::ffi::macros::__ffi_str(s.cross),
            }
        }
    };
}

// border::Set -> FfiBorderSet
#[macro_export]
macro_rules! ratatui_const_border_set_getter {
    ($fn_name:ident, $path:path) => {
        #[no_mangle]
        pub extern "C" fn $fn_name() -> crate::FfiBorderSet {
            let s = $path;
            crate::FfiBorderSet {
                top_left: $crate::ffi::macros::__ffi_str(s.top_left),
                top_right: $crate::ffi::macros::__ffi_str(s.top_right),
                bottom_left: $crate::ffi::macros::__ffi_str(s.bottom_left),
                bottom_right: $crate::ffi::macros::__ffi_str(s.bottom_right),
                vertical_left: $crate::ffi::macros::__ffi_str(s.vertical_left),
                vertical_right: $crate::ffi::macros::__ffi_str(s.vertical_right),
                horizontal_top: $crate::ffi::macros::__ffi_str(s.horizontal_top),
                horizontal_bottom: $crate::ffi::macros::__ffi_str(s.horizontal_bottom),
            }
        }
    };
}

// block::Set / bar::Set -> FfiLevelSet
#[macro_export]
macro_rules! ratatui_const_level_set_getter {
    ($fn_name:ident, $path:path) => {
        #[no_mangle]
        pub extern "C" fn $fn_name() -> crate::FfiLevelSet {
            let s = $path;
            crate::FfiLevelSet {
                full: $crate::ffi::macros::__ffi_str(s.full),
                seven_eighths: $crate::ffi::macros::__ffi_str(s.seven_eighths),
                three_quarters: $crate::ffi::macros::__ffi_str(s.three_quarters),
                five_eighths: $crate::ffi::macros::__ffi_str(s.five_eighths),
                half: $crate::ffi::macros::__ffi_str(s.half),
                three_eighths: $crate::ffi::macros::__ffi_str(s.three_eighths),
                one_quarter: $crate::ffi::macros::__ffi_str(s.one_quarter),
                one_eighth: $crate::ffi::macros::__ffi_str(s.one_eighth),
                empty: $crate::ffi::macros::__ffi_str(s.empty),
            }
        }
    };
}

// symbols::scrollbar::Set -> FfiScrollbarSet
#[macro_export]
macro_rules! ratatui_const_scrollbar_set_getter {
    ($fn_name:ident, $path:path) => {
        #[no_mangle]
        pub extern "C" fn $fn_name() -> crate::FfiScrollbarSet {
            let s = $path;
            crate::FfiScrollbarSet {
                track: $crate::ffi::macros::__ffi_str(s.track),
                thumb: $crate::ffi::macros::__ffi_str(s.thumb),
                begin: $crate::ffi::macros::__ffi_str(s.begin),
                end: $crate::ffi::macros::__ffi_str(s.end),
            }
        }
    };
}

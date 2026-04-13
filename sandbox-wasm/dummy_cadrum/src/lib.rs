use std::ffi::{CStr, c_char, c_double};
use std::io::{self, Write};

#[repr(C)]
pub struct Cube {
    pub sx: f64,
    pub sy: f64,
    pub sz: f64,
}

unsafe extern "C" {
    fn dummy_cube_to_step(sx: c_double, sy: c_double, sz: c_double) -> *mut c_char;
    fn dummy_free_cstring(s: *mut c_char);
}

pub fn cube() -> Cube {
    Cube { sx: 1.0, sy: 1.0, sz: 1.0 }
}

pub fn write_step<W: Write>(c: &Cube, w: &mut W) -> io::Result<()> {
    unsafe {
        let ptr = dummy_cube_to_step(c.sx, c.sy, c.sz);
        assert!(!ptr.is_null());
        let bytes = CStr::from_ptr(ptr).to_bytes().to_vec();
        dummy_free_cstring(ptr);
        w.write_all(&bytes)
    }
}

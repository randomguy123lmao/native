//! The few Lua C API calls the module makes, imported from Garry's Mod's own lua_shared.dll (LuaJIT 2.1).
//! Functions never raise Lua errors (a longjmp through Rust frames is undefined): they return nil and a message.

/// A value read from a Lua table (keys of a message's parameters).
#[derive(Clone)]
pub enum Val {
    Num(f64),
    Bool(bool),
    Str(String),
    Vec([f32; 3]),
    Other,
}

#[cfg(feature = "gmod")]
mod ffi {
    use super::Val;
    use std::ffi::{c_char, c_int, c_void};

    pub type State = *mut c_void;
    pub type CFunction = unsafe extern "C" fn(State) -> c_int;

    pub const GLOBALS: c_int = -10002;
    pub const TNIL: c_int = 0;
    pub const TBOOLEAN: c_int = 1;
    pub const TNUMBER: c_int = 3;
    pub const TSTRING: c_int = 4;
    pub const TTABLE: c_int = 5;

    #[link(name = "lua_shared", kind = "raw-dylib")]
    unsafe extern "C" {
        fn lua_gettop(l: State) -> c_int;
        fn lua_settop(l: State, idx: c_int);
        fn lua_type(l: State, idx: c_int) -> c_int;
        fn lua_tonumber(l: State, idx: c_int) -> f64;
        fn lua_toboolean(l: State, idx: c_int) -> c_int;
        fn lua_tolstring(l: State, idx: c_int, len: *mut usize) -> *const c_char;
        fn lua_objlen(l: State, idx: c_int) -> usize;
        fn lua_rawgeti(l: State, idx: c_int, n: c_int);
        fn lua_rawseti(l: State, idx: c_int, n: c_int);
        fn lua_createtable(l: State, narr: c_int, nrec: c_int);
        fn lua_setfield(l: State, idx: c_int, k: *const c_char);
        fn lua_getfield(l: State, idx: c_int, k: *const c_char);
        fn lua_pushnil(l: State);
        fn lua_pushnumber(l: State, n: f64);
        fn lua_pushboolean(l: State, b: c_int);
        fn lua_pushlstring(l: State, s: *const c_char, len: usize);
        fn lua_pushcclosure(l: State, f: CFunction, n: c_int);
        fn lua_pushvalue(l: State, idx: c_int);
        fn lua_next(l: State, idx: c_int) -> c_int;
        fn lua_touserdata(l: State, idx: c_int) -> *mut c_void;
    }

    pub const TUSERDATA: c_int = 7;

    /// One call's Lua state.
    #[derive(Clone, Copy)]
    pub struct Lua(pub State);

    impl Lua {
        pub fn top(self) -> i32 {
            unsafe { lua_gettop(self.0) }
        }

        pub fn kind(self, i: i32) -> i32 {
            unsafe { lua_type(self.0, i) }
        }

        pub fn num(self, i: i32) -> Option<f64> {
            (self.kind(i) == TNUMBER).then(|| unsafe { lua_tonumber(self.0, i) })
        }

        pub fn f32(self, i: i32) -> Result<f32, String> {
            self.num(i).map(|n| n as f32).ok_or_else(|| format!("argument {i}: a number expected"))
        }

        pub fn f32_or(self, i: i32, or: f32) -> f32 {
            self.num(i).map_or(or, |n| n as f32)
        }

        pub fn int(self, i: i32) -> Result<i32, String> {
            self.num(i).map(|n| n as i32).ok_or_else(|| format!("argument {i}: a number expected"))
        }

        pub fn vec3(self, i: i32) -> Result<[f32; 3], String> {
            Ok([self.f32(i)?, self.f32(i + 1)?, self.f32(i + 2)?])
        }

        pub fn boolean(self, i: i32) -> bool {
            unsafe { lua_toboolean(self.0, i) != 0 }
        }

        pub fn string(self, i: i32) -> Option<String> {
            if self.kind(i) != TSTRING {
                return None;
            }
            let mut len = 0usize;
            let p = unsafe { lua_tolstring(self.0, i, &mut len) };
            (!p.is_null()).then(|| String::from_utf8_lossy(unsafe { std::slice::from_raw_parts(p as *const u8, len) }).into_owned())
        }

        pub fn str_arg(self, i: i32) -> Result<String, String> {
            self.string(i).ok_or_else(|| format!("argument {i}: a string expected"))
        }

        /// An array table of numbers (1..n), as f32s.
        pub fn floats(self, i: i32) -> Result<Vec<f32>, String> {
            if self.kind(i) != TTABLE {
                return Err(format!("argument {i}: a table of numbers expected"));
            }
            let n = unsafe { lua_objlen(self.0, i) };
            let mut out = Vec::with_capacity(n);
            for k in 1..=n as c_int {
                unsafe { lua_rawgeti(self.0, i, k) };
                let v = self.num(-1);
                unsafe { lua_settop(self.0, -2) };
                out.push(v.ok_or_else(|| format!("argument {i}: entry {k} isn't a number"))? as f32);
            }
            Ok(out)
        }

        /// An array table (1..n) of Garry's Mod objects (userdata: a PhysObj, an Entity): what each wraps (the
        /// engine's pointer, the first thing in Garry's Mod's userdata), null for anything else.
        pub fn objects(self, i: i32) -> Result<Vec<*mut c_void>, String> {
            if self.kind(i) != TTABLE {
                return Err(format!("argument {i}: a table of objects expected"));
            }
            let n = unsafe { lua_objlen(self.0, i) };
            let mut out = Vec::with_capacity(n);
            for k in 1..=n as c_int {
                unsafe { lua_rawgeti(self.0, i, k) };
                let p = if self.kind(-1) == TUSERDATA {
                    let u = unsafe { lua_touserdata(self.0, -1) } as *const *mut c_void;
                    if u.is_null() { std::ptr::null_mut() } else { unsafe { *u } }
                } else {
                    std::ptr::null_mut()
                };
                unsafe { lua_settop(self.0, -2) };
                out.push(p);
            }
            Ok(out)
        }

        /// A table's string keys and their values (a message's parameters). Vectors are tables of 3 numbers.
        pub fn fields(self, i: i32) -> Result<Vec<(String, Val)>, String> {
            if self.kind(i) == TNIL {
                return Ok(vec![]);
            }
            if self.kind(i) != TTABLE {
                return Err(format!("argument {i}: a table expected"));
            }
            let i = if i < 0 { self.top() + i + 1 } else { i };
            let mut out = vec![];
            unsafe { lua_pushnil(self.0) };
            while unsafe { lua_next(self.0, i) } != 0 {
                // key at -2, value at -1
                if let Some(k) = self.string(-2) {
                    let v = match self.kind(-1) {
                        TNUMBER => Val::Num(unsafe { lua_tonumber(self.0, -1) }),
                        TBOOLEAN => Val::Bool(self.boolean(-1)),
                        TSTRING => Val::Str(self.string(-1).unwrap_or_default()),
                        TTABLE => match self.floats(-1) {
                            Ok(f) if f.len() == 3 => Val::Vec([f[0], f[1], f[2]]),
                            _ => self.xyz(-1).map_or(Val::Other, Val::Vec),
                        },
                        _ => Val::Other,
                    };
                    out.push((k, v));
                }
                unsafe { lua_settop(self.0, -2) };
            }
            Ok(out)
        }

        /// A table with x, y, z fields.
        fn xyz(self, i: i32) -> Option<[f32; 3]> {
            let mut v = [0.0f32; 3];
            for (k, name) in [c"x", c"y", c"z"].iter().enumerate() {
                unsafe { lua_getfield(self.0, i, name.as_ptr()) };
                let n = self.num(-1);
                unsafe { lua_settop(self.0, -2) };
                v[k] = n? as f32;
            }
            Some(v)
        }

        pub fn push_nil(self) {
            unsafe { lua_pushnil(self.0) }
        }

        pub fn push_num(self, n: f64) {
            unsafe { lua_pushnumber(self.0, n) }
        }

        pub fn push_bool(self, b: bool) {
            unsafe { lua_pushboolean(self.0, b as c_int) }
        }

        pub fn push_str(self, s: &str) {
            unsafe { lua_pushlstring(self.0, s.as_ptr() as *const c_char, s.len()) }
        }

        /// An array table of numbers.
        pub fn push_floats(self, f: &[f32]) {
            unsafe { lua_createtable(self.0, f.len() as c_int, 0) };
            for (k, v) in f.iter().enumerate() {
                unsafe {
                    lua_pushnumber(self.0, *v as f64);
                    lua_rawseti(self.0, -2, k as c_int + 1);
                }
            }
        }

        /// An array table of strings.
        pub fn push_strings<S: AsRef<str>>(self, s: &[S]) {
            unsafe { lua_createtable(self.0, s.len() as c_int, 0) };
            for (k, v) in s.iter().enumerate() {
                self.push_str(v.as_ref());
                unsafe { lua_rawseti(self.0, -2, k as c_int + 1) };
            }
        }

        pub fn new_table(self) {
            unsafe { lua_createtable(self.0, 0, 8) }
        }

        /// t[k] = the value on top (t at -2), popping it.
        pub fn set_field(self, k: &std::ffi::CStr) {
            unsafe { lua_setfield(self.0, -2, k.as_ptr()) }
        }

        /// t[n] = the value on top (t at -2), popping it.
        pub fn set_index(self, n: i32) {
            unsafe { lua_rawseti(self.0, -2, n) }
        }

        pub fn field_num(self, k: &std::ffi::CStr, v: f64) {
            self.push_num(v);
            self.set_field(k);
        }

        pub fn field_bool(self, k: &std::ffi::CStr, v: bool) {
            self.push_bool(v);
            self.set_field(k);
        }

        pub fn field_str(self, k: &std::ffi::CStr, v: &str) {
            self.push_str(v);
            self.set_field(k);
        }

        /// The global table `name` holding these functions.
        pub fn register(self, name: &std::ffi::CStr, fns: &[(&std::ffi::CStr, CFunction)]) {
            unsafe {
                lua_createtable(self.0, 0, fns.len() as c_int);
                for (n, f) in fns {
                    lua_pushcclosure(self.0, *f, 0);
                    lua_setfield(self.0, -2, n.as_ptr());
                }
                lua_pushvalue(self.0, -1);
                lua_setfield(self.0, GLOBALS, name.as_ptr());
                lua_settop(self.0, -2);
            }
        }
    }

    /// A Lua-callable function: its body returns how many values it pushed, or an error (pushed as nil and the message).
    /// Panics are caught here: unwinding into Garry's Mod would take the server down.
    #[macro_export]
    macro_rules! lua_fn {
        ($name:ident, |$l:ident| $body:block) => {
            unsafe extern "C" fn $name(state: $crate::lua::State) -> std::ffi::c_int {
                let $l = $crate::lua::Lua(state);
                let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| -> Result<std::ffi::c_int, String> { $body }));
                match r {
                    Ok(Ok(n)) => n,
                    Ok(Err(e)) => {
                        $l.push_nil();
                        $l.push_str(&e);
                        2
                    }
                    Err(p) => {
                        let why = p.downcast_ref::<&str>().map(|s| s.to_string()).or_else(|| p.downcast_ref::<String>().cloned()).unwrap_or_default();
                        $l.push_nil();
                        $l.push_str(&format!("delirium: internal error in {}: {why}", stringify!($name)));
                        2
                    }
                }
            }
        };
    }

    pub fn args(l: Lua) -> i32 {
        l.top()
    }

    #[allow(dead_code)]
    pub fn null() -> *mut c_void {
        std::ptr::null_mut()
    }
}
#[cfg(feature = "gmod")]
pub use ffi::*;

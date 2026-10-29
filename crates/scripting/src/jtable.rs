//! Reading Janet tables and structs from C functions.
//!
//! `conv` covers scalars; the registration APIs (templates, toolchains,
//! language specs) take nested spec arguments, and those arrive as either
//! tables (`@{...}`) or structs (`{...}` literals — what manifest files
//! write).  Every accessor here accepts both.  All functions are `unsafe`
//! and expect a live Janet value from the current VM — same contract as
//! `conv`.

use evil_janet::*;

/// True for table and struct values — the two spec shapes.
pub unsafe fn is_spec(v: Janet) -> bool {
    unsafe {
        janet_checktype(v, JanetType_JANET_TABLE) != 0
            || janet_checktype(v, JanetType_JANET_STRUCT) != 0
    }
}

fn make_key(key: &str) -> Janet {
    unsafe { janet_wrap_keyword(janet_symbol(key.as_ptr(), key.len() as i32)) }
}

/// `spec[key]` as a raw Janet value; nil when absent or the spec is
/// neither table nor struct.
pub unsafe fn get(v: Janet, key: &str) -> Janet {
    unsafe {
        match janet_checktype(v, JanetType_JANET_TABLE) != 0 {
            true => janet_table_get(janet_unwrap_table(v), make_key(key)),
            false => {
                if janet_checktype(v, JanetType_JANET_STRUCT) == 0 {
                    return janet_wrap_nil();
                }
                let kv = janet_struct_find(janet_unwrap_struct(v), make_key(key));
                if kv.is_null() {
                    janet_wrap_nil()
                } else {
                    (*kv).value
                }
            }
        }
    }
}

/// `spec[key]` as a string/keyword, or None.
pub unsafe fn get_str(v: Janet, key: &str) -> Option<String> {
    unsafe {
        let mut argv = get(v, key);
        super::conv::get_str(1, &mut argv, 0)
    }
}

/// `spec[key]` as a nested table/struct value, or None.
pub unsafe fn get_spec(v: Janet, key: &str) -> Option<Janet> {
    unsafe {
        let v = get(v, key);
        if is_spec(v) { Some(v) } else { None }
    }
}

/// `spec[key]` as a boolean, or None.
pub unsafe fn get_bool(v: Janet, key: &str) -> Option<bool> {
    unsafe {
        let mut argv = get(v, key);
        super::conv::get_bool(1, &mut argv, 0)
    }
}

/// `spec[key]` as an array/tuple of strings/keywords, or None.
pub unsafe fn get_str_array(v: Janet, key: &str) -> Option<Vec<String>> {
    unsafe {
        let v = get(v, key);
        let is_array = janet_checktype(v, JanetType_JANET_ARRAY) != 0;
        let is_tuple = janet_checktype(v, JanetType_JANET_TUPLE) != 0;
        if !is_array && !is_tuple {
            return None;
        }
        let count = janet_length(v) as usize;
        let mut out = Vec::with_capacity(count);
        for i in 0..count {
            let elem = if is_array {
                *(*janet_unwrap_array(v)).data.add(i)
            } else {
                *janet_unwrap_tuple(v).add(i)
            };
            let mut argv = elem;
            if let Some(s) = super::conv::get_str(1, &mut argv, 0) {
                out.push(s);
            }
        }
        Some(out)
    }
}

/// A string-keyed spec of table/struct values, iterated in backing order:
/// useful for `{:variables {"greeting" {...}}}`-shaped specs.  Non-(string,
/// spec) entries are skipped.
pub unsafe fn string_spec_entries(v: Janet) -> Vec<(String, Janet)> {
    unsafe {
        let mut out = Vec::new();
        if janet_checktype(v, JanetType_JANET_TABLE) != 0 {
            let tbl = janet_unwrap_table(v);
            // Live entries sit at their hash index anywhere in [0, capacity).
            for idx in 0..(*tbl).capacity {
                let kv = *(*tbl).data.add(idx as usize);
                if janet_checktype(kv.key, JanetType_JANET_NIL) != 0 {
                    continue;
                }
                let mut key_argv = kv.key;
                if let Some(k) = super::conv::get_str(1, &mut key_argv, 0) {
                    out.push((k, kv.value));
                }
            }
            return out;
        }
        if janet_checktype(v, JanetType_JANET_STRUCT) != 0 {
            let head = janet_struct_head(janet_unwrap_struct(v));
            let head = &*head;
            let st = janet_unwrap_struct(v);
            for idx in 0..head.capacity {
                let kv = *st.add(idx as usize);
                if janet_checktype(kv.key, JanetType_JANET_NIL) != 0 {
                    continue;
                }
                let mut key_argv = kv.key;
                if let Some(k) = super::conv::get_str(1, &mut key_argv, 0) {
                    out.push((k, kv.value));
                }
            }
        }
        out
    }
}

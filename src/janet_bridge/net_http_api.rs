//! Janet C functions for HTTP requests (Sprint 13).

use evil_janet::*;
use super::conv;
use super::with_editor;

unsafe extern "C-unwind" fn c_http_request(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| unsafe {
        let method = match conv::get_str(argc, argv, 0) {
            Some(s) => s,
            None => conv::signal_err("net/http-request: method string required"),
        };
        let url = match conv::get_str(argc, argv, 1) {
            Some(s) => s,
            None => conv::signal_err("net/http-request: url string required"),
        };
        let body = conv::get_str(argc, argv, 2).unwrap_or_default();
        let bg = match ed.background.as_ref() {
            Some(bg) => bg.clone(),
            None => conv::signal_err("net/http-request: no background runtime"),
        };
        let id = ed.io.next_net_id;
        ed.io.next_net_id += 1;
        let sender = bg.sender.clone();
        debug_http!("→ id={id} {method} {url} body_len={}", body.len());
        bg.spawn_blocking(move || {
            let result = (|| -> Result<(u16, String), String> {
                let req = ureq::request(&method, &url);
                let resp = if body.is_empty() {
                    req.call().map_err(|e| e.to_string())?
                } else {
                    req.send_string(&body).map_err(|e| e.to_string())?
                };
                let status = resp.status();
                let text = resp.into_string().map_err(|e| e.to_string())?;
                Ok((status, text))
            })();
            match result {
                Ok((status, body)) => {
                    debug_http!("← id={id} {status} body_len={}", body.len());
                    let _ = sender.send(crate::runtime::BackgroundEvent::HttpResponse { id, status, body });
                }
                Err(error) => {
                    debug_http!("✗ id={id} {error}");
                    let _ = sender.send(crate::runtime::BackgroundEvent::HttpError { id, error });
                }
            }
        });
        conv::integer(id as i32)
    })
}

unsafe extern "C-unwind" fn c_http_get(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| unsafe {
        let url = match conv::get_str(argc, argv, 0) {
            Some(s) => s,
            None => conv::signal_err("net/http-get: url string required"),
        };
        let bg = match ed.background.as_ref() {
            Some(bg) => bg.clone(),
            None => conv::signal_err("net/http-get: no background runtime"),
        };
        let id = ed.io.next_net_id;
        ed.io.next_net_id += 1;
        let sender = bg.sender.clone();
        debug_http!("→ id={id} GET {url}");
        bg.spawn_blocking(move || {
            let result = (|| -> Result<(u16, String), String> {
                let resp = ureq::get(&url).call().map_err(|e| e.to_string())?;
                let status = resp.status();
                let text = resp.into_string().map_err(|e| e.to_string())?;
                Ok((status, text))
            })();
            match result {
                Ok((status, body)) => {
                    debug_http!("← id={id} {status} body_len={}", body.len());
                    let _ = sender.send(crate::runtime::BackgroundEvent::HttpResponse { id, status, body });
                }
                Err(error) => {
                    debug_http!("✗ id={id} {error}");
                    let _ = sender.send(crate::runtime::BackgroundEvent::HttpError { id, error });
                }
            }
        });
        conv::integer(id as i32)
    })
}

unsafe extern "C-unwind" fn c_http_post(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| unsafe {
        let url = match conv::get_str(argc, argv, 0) {
            Some(s) => s,
            None => conv::signal_err("net/http-post: url string required"),
        };
        let body = conv::get_str(argc, argv, 1).unwrap_or_default();
        let bg = match ed.background.as_ref() {
            Some(bg) => bg.clone(),
            None => conv::signal_err("net/http-post: no background runtime"),
        };
        let id = ed.io.next_net_id;
        ed.io.next_net_id += 1;
        let sender = bg.sender.clone();
        debug_http!("→ id={id} POST {url} body_len={}", body.len());
        bg.spawn_blocking(move || {
            let result = (|| -> Result<(u16, String), String> {
                let resp = ureq::post(&url).send_string(&body).map_err(|e| e.to_string())?;
                let status = resp.status();
                let text = resp.into_string().map_err(|e| e.to_string())?;
                Ok((status, text))
            })();
            match result {
                Ok((status, body)) => {
                    debug_http!("← id={id} {status} body_len={}", body.len());
                    let _ = sender.send(crate::runtime::BackgroundEvent::HttpResponse { id, status, body });
                }
                Err(error) => {
                    debug_http!("✗ id={id} {error}");
                    let _ = sender.send(crate::runtime::BackgroundEvent::HttpError { id, error });
                }
            }
        });
        conv::integer(id as i32)
    })
}

pub fn register() -> Vec<JanetReg> {
    vec![
        JanetReg {
            name: c"net/http-request".as_ptr(),
            cfun: Some(c_http_request as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"Fire an HTTP request; emits http-response or http-error event".as_ptr(),
        },
        JanetReg {
            name: c"net/http-get".as_ptr(),
            cfun: Some(c_http_get as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"GET the given URL; emits http-response or http-error event".as_ptr(),
        },
        JanetReg {
            name: c"net/http-post".as_ptr(),
            cfun: Some(c_http_post as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"POST body to the given URL; emits http-response or http-error event".as_ptr(),
        },
    ]
}

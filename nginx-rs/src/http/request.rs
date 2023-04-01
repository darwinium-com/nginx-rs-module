use crate::{bindings::*, ngx_null_string};
use crate::core::*;
use crate::http::status::*;

use std::os::raw::c_void;

/// Define a static request handler.
///
/// Handlers are expected to take a single [`Request`] argument and return a [`Status`].
#[macro_export]
macro_rules! http_request_handler {
    ( $name: ident, $handler: expr ) => {
        #[no_mangle]
        extern "C" fn $name(r: *mut ngx_http_request_t) -> ngx_int_t {
            let status: Status = $handler(unsafe { &mut $crate::http::Request::from_ngx_http_request(r) });
            status.0
        }
    };
}

#[repr(transparent)]
pub struct Request(ngx_http_request_t);

impl Request {
    /// Create a [`Request`] from an [`ngx_http_request_t`].
    ///
    /// [`ngx_http_request_t`]: https://nginx.org/en/docs/dev/development_guide.html#http_request
    pub unsafe fn from_ngx_http_request<'a>(r: *mut ngx_http_request_t) -> &'a mut Request {
        // SAFETY: The caller has provided a valid non-null pointer to a valid `ngx_http_request_t`
        // which shares the same representation as `Request`.
        &mut *r.cast::<Request>()
    }

    /// Is this the main request (as opposed to a subrequest)?
    pub fn is_main(&self) -> bool {
        let main = self.0.main.cast();
        std::ptr::eq(self, main)
    }

    /// Request pool.
    pub fn pool(&self) -> Pool {
        // SAFETY: This request is allocated from `pool`, thus must be a valid pool.
        unsafe {
            Pool::from_ngx_pool(self.0.pool)
        }
    }

    /// Pointer to a [`ngx_connection_t`] client connection object.
    ///
    /// [`ngx_connection_t`]: https://nginx.org/en/docs/dev/development_guide.html#connection
    pub fn connection(&self) -> *mut ngx_connection_t {
        self.0.connection
    }

    pub fn remote_address(&self) -> Option<String> {
        unsafe {
            let connection = self.0.connection;
            let sockaddr = (*connection).sockaddr;
            let socklen = (*connection).socklen;
            const IP_MAX_LEN: u64 = 128;  // should be enough for ipv4 and ipv6
            let p = ngx_pnalloc(self.0.pool, IP_MAX_LEN);
            if p.is_null() {
                None
            } else {
                let buf = p as *mut u8;
                let len = ngx_sock_ntop(sockaddr, socklen, buf, IP_MAX_LEN, 1);
                let s = std::slice::from_raw_parts(buf, len as usize);
                let addr = String::from_utf8_lossy(s);
                if addr.is_empty() {
                    None
                } else {
                    Some(addr.to_string())
                }
            }
        }
    }

    /// Module location configuration.
    pub fn get_module_loc_conf(&self, module: &ngx_module_t) -> *mut c_void {
        unsafe {
            *self.0.loc_conf.add(module.ctx_index)
        }
    }

    /// main configuration.
    pub fn get_module_main_conf(&self, module: &ngx_module_t) -> *mut c_void {
        unsafe {
            *self.0.main_conf.add(module.ctx_index)
        }
    }

    /// Get the value of a [complex value].
    ///
    /// [complex value]: https://nginx.org/en/docs/dev/development_guide.html#http_complex_values
    pub fn get_complex_value(&self, cv: &ngx_http_complex_value_t) -> Option<&NgxStr> {
        let r = (self as *const Request as *mut Request).cast();
        let val = cv as *const ngx_http_complex_value_t as *mut ngx_http_complex_value_t;
        // SAFETY: `ngx_http_complex_value` does not mutate `r` or `val` and guarentees that
        // a valid Nginx string is stored in `value` if it successfully returns.
        unsafe {
            let mut value = ngx_null_string!();
            if ngx_http_complex_value(r, val, &mut value) != NGX_OK as ngx_int_t {
                return None;
            }
            Some(NgxStr::from_ngx_str(value))
        }
    }

    /// Discard (read and ignore) the [request body].
    ///
    /// [request body]: https://nginx.org/en/docs/dev/development_guide.html#http_request_body
    pub fn discard_request_body(&mut self) -> Status
    {
        unsafe {
            Status(ngx_http_discard_request_body(&mut self.0))
        }
    }

    /// Client HTTP [User-Agent].
    ///
    /// [User-Agent]: https://developer.mozilla.org/en-US/docs/Web/HTTP/Headers/User-Agent
    pub fn user_agent(&self) -> &NgxStr {
        unsafe {
            NgxStr::from_ngx_str((*self.0.headers_in.user_agent).value)
        }
    }

    fn get_value(header: *const ngx_table_elt_t) -> Option<String> {
        if header.is_null() {
            None
        } else {
            let value = unsafe { NgxStr::from_ngx_str((*header).value).to_string_lossy().to_string() };
            if value.is_empty() {
                None
            } else {
                Some(value)
            }
        }
    }

    fn get_value_from_part(headers: ngx_list_t, header_name: &str) -> Option<String> {
        unsafe {
            let mut part = headers.part;
            let mut h = part.elts as *mut ngx_table_elt_t;
            let mut i = 0;
            let mut name = header_name.to_string();
            loop {
                if i >= part.nelts {
                    if part.next.is_null() {
                        break;
                    }
                    part = *part.next;
                    h = part.elts as *mut ngx_table_elt_t;
                    i = 0;
                }
                let header = *h.add(i);
                if ngx_strncasecmp(header.key.data, name.as_mut_ptr(), header.key.len) != 0 {
                    i += 1;
                    continue;
                }
                let s = std::slice::from_raw_parts(header.value.data, header.value.len as usize);
                let name = String::from_utf8_lossy(s);
                return Some(name.to_string());
            }
            None
        }
    }

    pub fn get_header_names(&self) -> Option<String> {
        unsafe {
            let mut part = self.0.headers_in.headers.part;
            let mut h = part.elts as *mut ngx_table_elt_t;
            let mut i = 0;
            let mut h_vec = Vec::new();
            loop {
                if i >= part.nelts {
                    if part.next.is_null() {
                        break;
                    }
                    part = *part.next;
                    h = part.elts as *mut ngx_table_elt_t;
                    i = 0;
                }
                let header = *h.add(i);
                let s = std::slice::from_raw_parts(header.key.data, header.key.len as usize);
                let name = String::from_utf8_lossy(s);
                if !name.is_empty() {
                    h_vec.push(name);
                }
                /* only names, no values
                let s = std::slice::from_raw_parts(header.value.data, header.value.len as usize);
                let value =  String::from_utf8_lossy(s);
                */
                // TODO we may need values in hash at some point, but need filter out some values
                // which change all the time
                i += 1;
            }
            if h_vec.is_empty() {
                None
            } else {
                let headers = h_vec.join(" ");
                Some(headers)
            }
        }
    }

    pub fn get_header(&self, header: &str) -> Option<String> {
        let lower = header.to_ascii_lowercase();
        let header = lower.as_str();
        match header {
            "host" => Self::get_value(self.0.headers_in.host),
            "user-agent" | "user_agent" => Self::get_value(self.0.headers_in.user_agent),
            "referer" => Self::get_value(self.0.headers_in.referer),
            "accept_language" | "accept-language" => Self::get_value(self.0.headers_in.accept_language),
            "content-type" | "content_type" => Self::get_value(self.0.headers_in.content_type),
            "content-length" | "content_length" => Self::get_value(self.0.headers_in.content_length),
            "accept" => Self::get_value(self.0.headers_in.accept),
            _ => Self::get_value_from_part(self.0.headers_in.headers, header),
        }
    }

    pub fn set_header(&self, name: &str, value: &str) {
        unsafe {
            let mut pool = self.pool();
            let n = pool.allocate::<String>(name.to_string()) as *mut u8;
            let v = pool.allocate::<String>(value.to_string()) as *mut u8;

            let n_str = ngx_str_t { len: name.len() as u64, data: n };
            let v_str = ngx_str_t { len: value.len() as u64, data: v };

            let mut headers = self.0.headers_out.headers;
            let mut h = ngx_list_push(&mut headers as *mut ngx_list_t) as *mut ngx_table_elt_t;

            (*h).hash = 1;
            (*h).key = n_str;
            (*h).value = v_str;
        }
    }

    /// Set HTTP status of response.
    pub fn set_status(&mut self, status: HTTPStatus) {
        self.0.headers_out.status = status.into();
    }

    /// Set response body [Content-Length].
    ///
    /// [Content-Length]: https://developer.mozilla.org/en-US/docs/Web/HTTP/Headers/Content-Length
    pub fn set_content_length_n(&mut self, n: usize) {
        self.0.headers_out.content_length_n = n as off_t;
    }

    /// Send the output header.
    ///
    /// Do not call this function until all output headers are set.
    pub fn send_header(&mut self) -> Status {
        unsafe {
            Status(ngx_http_send_header(&mut self.0))
        }
    }

    /// Flag indicating that the output does not require a body.
    ///
    /// For example, this flag is used by `HTTP HEAD` requests.
    pub fn header_only(&self) -> bool {
        self.0.header_only() != 0
    }

    /// Send the [response body].
    ///
    /// This function can be called multiple times.
    /// Set the `last_buf` flag in the last body buffer.
    ///
    /// [response body]: https://nginx.org/en/docs/dev/development_guide.html#http_request_body
    pub fn output_filter(&mut self, body: &mut ngx_chain_t) -> Status {
        unsafe {
            Status(ngx_http_output_filter(&mut self.0, body))
        }
    }

    pub fn uri(&self) -> &NgxStr {
        unsafe {
            NgxStr::from_ngx_str(self.0.uri)
        }
    }

    pub fn args(&self) -> &NgxStr {
        unsafe {
            NgxStr::from_ngx_str(self.0.args)
        }
    }

    pub fn addr(&self) -> &NgxStr {
        unsafe {
            NgxStr::from_ngx_str((*self.0.connection).addr_text)
        }
    }
}

use crate::bindings::*;

use std::os::raw::c_void;
use core::ptr;

pub unsafe fn ngx_http_conf_get_module_main_conf(cf: *mut ngx_conf_t, module: &ngx_module_t)  -> *mut c_void {
    let http_conf_ctx = (*cf).ctx as *mut ngx_http_conf_ctx_t;
    *(*http_conf_ctx).main_conf.add(module.ctx_index)
}

pub unsafe fn ngx_http_conf_get_module_srv_conf(cf: *mut ngx_conf_t, module: &ngx_module_t)  -> *mut c_void {
    let http_conf_ctx = (*cf).ctx as *mut ngx_http_conf_ctx_t;
    *(*http_conf_ctx).srv_conf.add(module.ctx_index)
}

pub unsafe fn ngx_http_conf_get_module_loc_conf(cf: *mut ngx_conf_t, module: &ngx_module_t)  -> *mut c_void {
    let http_conf_ctx = (*cf).ctx as *mut ngx_http_conf_ctx_t;
    *(*http_conf_ctx).loc_conf.add(module.ctx_index)
}

pub unsafe fn ngx_cycle_conf_get_module_main_conf(cycle: *mut ngx_cycle_t, module: &ngx_module_t)  -> *mut c_void {
    let idx = ngx_http_module.index;
    let http_conf_ctx = *((*cycle).conf_ctx.add(idx)) as *mut ngx_http_conf_ctx_t ;
    if http_conf_ctx.is_null() {
        ptr::null_mut()
    } else {
        *(*http_conf_ctx).main_conf.add(module.ctx_index)
    }
}

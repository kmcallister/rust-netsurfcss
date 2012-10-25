use core::libc::{c_void, size_t};
use core::libc::types::common::c99::{int32_t, uint64_t};
use ll::errors::*;
use ll::stylesheet::*;
use ll::types::*;
use ll::select::*;
use ll::properties::{css_font_style_e, css_font_variant_e, css_font_weight_e};
use ll_css_stylesheet_create = ll::stylesheet::css_stylesheet_create;
use ll_css_select_ctx_create = ll::select::css_select_ctx_create;
use ptr::{null, to_unsafe_ptr, to_mut_unsafe_ptr};
use cast::transmute;

use wapcaplet::ll::lwc_string;

pub type CssStylesheet = c_void;

pub enum CssLanguageLevel {
    CssLevel1 = 0,
    CssLevel2 = 1,
    CssLevel21 = 2,
    CssLevel3 = 3,
    CssLevelDefault = 99 // NB: This is not the same as the ll value
}

pub struct CssStylesheetParams {
    params_version: CssStylesheetParamsVersion,
    level: CssLanguageLevel,
    charset: ~str,
    url: ~str,
    title: ~str,
    allow_quirks: bool,
    inline_style: bool,
    resolve: Option<CssUrlResolutionFn>,
    import: Option<CssImportNotificationFn>,
    color: Option<CssColorResolutionFn>,
    font: Option<CssFontResolutionFn>,
}

pub enum CssStylesheetParamsVersion {
    CssStylesheetParamsVersion1 = 1
}

// FIXME: Need hl reprs of lwc_string
pub type CssUrlResolutionFn = ~fn(base: &str, rel: &lwc_string, abs: & &lwc_string) -> css_error;
pub type CssImportNotificationFn = ~fn(parent: &CssStylesheet, url: &lwc_string, media: &uint64_t) -> css_error;
pub type CssColorResolutionFn = ~fn(name: &lwc_string, color: &CssColor) -> css_error;
pub type CssFontResolutionFn = ~fn(name: &lwc_string, system_font: &CssSystemFont) -> css_error;

pub struct CssColor { r: u8, g: u8, b: u8, a: u8 }

pub struct CssSystemFont {
    style: css_font_style_e,
    variant: css_font_variant_e,
    weight: css_font_weight_e,
    size: css_size,
    line_height: css_size,
    family: ~str
}

fn ll_result_to_rust_result<T>(code: css_error, val: T) -> CssResult<T> {
    match code {
        CSS_OK => Ok(move val),
        _ => Err(unsafe { transmute(code) })
    }
}

type CssResult<T> = Result<T, css_error>;

fn require_ok(code: css_error, what: &str) {
    match code {
        CSS_OK => (),
        e => fail fmt!("CSS parsing failed while %s. code: %?", what, e)
    }
}

pub struct CssStylesheetRef {
    priv params: CssStylesheetParams,
    priv sheet: *css_stylesheet,

    drop {
        assert self.sheet.is_not_null();
        css_stylesheet_destroy(self.sheet);
    }
}

fn css_stylesheet_create(params: CssStylesheetParams) -> CssStylesheetRef {
    let sheet = do params.as_ll |ll_params| {
        let mut sheet: *css_stylesheet = null();
        let code = ll_css_stylesheet_create(
            to_unsafe_ptr(ll_params), realloc, null(), to_mut_unsafe_ptr(&mut sheet));
        require_ok(code, "creating stylesheet");
        assert sheet.is_not_null();
        sheet
    };

    CssStylesheetRef {
        // Store the params to keep their pointers alive
        params: move params,
        sheet: sheet
    }
}

impl CssStylesheetRef {
    fn size() -> uint {
        let mut size = 0;
        let code = css_stylesheet_size(self.sheet, to_mut_unsafe_ptr(&mut size));
        require_ok(code, "getting stylesheet size");
        return size as uint;
    }

    fn append_data(data: &[u8]) {
        // FIXME: For some reason to_const_ptr isn't accessible
        let code = css_stylesheet_append_data(self.sheet, unsafe { transmute(vec::raw::to_ptr(data)) }, data.len() as size_t);
        match code {
            CSS_NEEDDATA => { /* fine */ },
            _ => require_ok(code, "appending styleshet data")
        }
    }

    fn data_done() {
        let code = css_stylesheet_data_done(self.sheet);
        require_ok(code, "finishing parsing");
    }
}

extern fn realloc(ptr: *c_void, len: size_t, _pw: *c_void) -> *c_void {
    libc::realloc(ptr, len)
}

struct CssSelectCtxRef {
    priv select_ctx: *css_select_ctx,
    // Whenever a sheet is added to the select ctx we will take ownership of it
    // to ensure that it stays alive
    priv mut sheets: ~[CssStylesheetRef],

    drop {
        assert self.select_ctx.is_not_null();
        css_select_ctx_destroy(self.select_ctx);
    }
}

fn css_select_ctx_create() -> CssSelectCtxRef {
    let mut select_ctx: *css_select_ctx = null();
    let code = ll_css_select_ctx_create(realloc, null(), to_mut_unsafe_ptr(&mut select_ctx));
    require_ok(code, "creating select context");
    assert select_ctx.is_not_null();

    CssSelectCtxRef {
        select_ctx: select_ctx,
        mut sheets: ~[]
    }
}

impl CssSelectCtxRef {
    fn append_sheet(sheet: CssStylesheetRef, origin: css_origin, media: uint64_t) {
        let code = css_select_ctx_append_sheet(self.select_ctx, sheet.sheet, origin, media);
        require_ok(code, "adding sheet to select ctx");

        self.sheets.push(move sheet);
    }
    fn count_sheets() -> uint {
        let mut count = 0;
        let code = css_select_ctx_count_sheets(self.select_ctx, to_mut_unsafe_ptr(&mut count));
        require_ok(code, "counting sheets");
        return count as uint;
    }
}
#[macro_use]
extern crate neon;
extern crate merk;

use std::path::Path;
use neon::prelude::*;

fn open(mut cx: FunctionContext) -> JsResult<JsString> {
    let path = cx.argument::<JsString>(0)?;
    Ok(cx.string("hello node"))
}

register_module!(mut cx, {
    cx.export_function("open", open)
});

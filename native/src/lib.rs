#[macro_use]
extern crate neon;
extern crate merk;

use std::path::Path;
use merk::Merk;
use neon::prelude::*;

pub struct MerkHandle {
    store: Merk
}

// TODO: throw instead of panicking

declare_types! {
  pub class JsMerk for MerkHandle {
    init(mut cx) {
        let path = cx.argument::<JsString>(0)?.value();
        let path = Path::new(&path);
        let store = Merk::open(path).unwrap();
        Ok(MerkHandle { store })
    }

    method get(mut cx) {
        let key = cx.argument::<JsBuffer>(0)?;
        let this = cx.this();
        let value = cx.borrow(&key, |key| {
            let guard = cx.lock();
            let handle = this.borrow(&guard);
            handle.store.get(key.as_slice()).unwrap()
        });

        let buffer = cx.buffer(value.len() as u32)?;
        for i in 0..value.len() {
            let n = cx.number(value[i]);
            buffer.set(&mut cx, i as u32, n)?;
        }
        Ok(buffer.upcast())
    }
  }
}
register_module!(mut m, { m.export_class::<JsMerk>("Merk") });

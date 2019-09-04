#[macro_use]
extern crate neon;
extern crate merk;

use std::collections::BTreeMap;
use std::path::Path;
use merk::{Merk, Op};
use neon::prelude::*;

pub struct MerkHandle {
    store: Merk
}

pub struct Batch {
    ops: BTreeMap<Vec<u8>, Op>
}

// TODO: throw instead of panicking


macro_rules! buffer_arg_to_vec {
    ($cx:ident, $index:expr) => {
        {
            let buffer = $cx.argument::<JsBuffer>($index)?;
            $cx.borrow(
                &buffer,
                |buffer| buffer.as_slice().to_vec()
            )
        }
    };
}

declare_types! {
    pub class JsMerk for MerkHandle {
        init(mut cx) {
            let path = cx.argument::<JsString>(0)?.value();
            let path = Path::new(&path);
            let store = Merk::open(path).unwrap();
            Ok(MerkHandle { store })
        }

        method get_sync(mut cx) {
            let key = buffer_arg_to_vec!(cx, 0);

            let value = {
                let this = cx.this();
                let guard = cx.lock();
                let handle = this.borrow(&guard);
                handle.store.get(key.as_slice()).unwrap()
            };

            let buffer = cx.buffer(value.len() as u32)?;
            for i in 0..value.len() {
                let n = cx.number(value[i]);
                buffer.set(&mut cx, i as u32, n)?;
            }
            Ok(buffer.upcast())
        }

        method batch(mut cx) {
            let args: Vec<Handle<JsValue>> = vec![];
            Ok(JsBatch::new(&mut cx, args)?.upcast())
        }
    }

    pub class JsBatch for Batch {
        init(_cx) {
            Ok(Batch { ops: BTreeMap::new() })
        }

        method put(mut cx) {
            {
                let key = buffer_arg_to_vec!(cx, 0);
                let value = buffer_arg_to_vec!(cx, 1);
                // TODO: assert lengths


                let mut this = cx.this();
                let guard = cx.lock();
                let mut handle = this.borrow_mut(&guard);
                handle.ops.insert(key, Op::Put(value));
            }

            Ok(cx.undefined().upcast())
        }

        method delete(mut cx) {
            {
                let key = buffer_arg_to_vec!(cx, 0);
                // TODO: assert length

                let mut this = cx.this();
                let guard = cx.lock();
                let mut handle = this.borrow_mut(&guard);
                handle.ops.insert(key, Op::Delete);
            }

            Ok(cx.undefined().upcast())
        }

        method commit_sync(mut cx) {
            cx.throw_error("not yet implemented")
        }

        method commit(mut cx) {
            cx.throw_error("not yet implemented")
        }
    }
}

register_module!(mut m, {
    m.export_class::<JsMerk>("Merk")
});

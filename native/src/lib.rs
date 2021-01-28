extern crate failure;
extern crate merk;
extern crate neon;

use merk::{Merk, Op};
use neon::prelude::*;
use std::collections::BTreeMap;
use std::ops::DerefMut;
use std::path::Path;
use std::rc::Rc;
use std::sync::Mutex;

pub struct MerkHandle {
    pub store: Rc<Mutex<Option<Merk>>>,
}

pub struct Batch {
    ops: Option<BTreeMap<Vec<u8>, Op>>,
    store: Rc<Mutex<Option<Merk>>>,
}

// TODO: throw instead of panicking
// TODO: make this code succinct

macro_rules! buffer_arg_to_vec {
    ($cx:ident, $index:expr) => {{
        let buffer = $cx.argument::<JsBuffer>($index)?;
        $cx.borrow(&buffer, |buffer| buffer.as_slice().to_vec())
    }};
}

macro_rules! borrow_store {
    ($cx:ident, $op:expr) => {{
        let res = {
            let this = $cx.this();
            let guard = $cx.lock();
            let handle = this.borrow(&guard);
            let res = handle.store.lock();
            match res {
                Err(_err) => panic!("failed to acquire lock"),
                Ok(mut store) => ($op)(store.deref_mut().as_mut().expect("Merk is closed")),
            }
        };
        match res {
            Err(err) => panic!(err),
            Ok(value) => value,
        }
    }};
}

declare_types! {
    pub class JsMerk for MerkHandle {
        init(mut cx) {
            let path = cx.argument::<JsString>(0)?.value();
            let path = Path::new(&path);
            match Merk::open(path) {
                Err(_err) => cx.throw_error("failed to open merk store"),
                Ok(store) => Ok(MerkHandle {
                    store: Rc::new(Mutex::new(Some(store)))
                })
            }
        }

        method getSync(mut cx) {
            let key = buffer_arg_to_vec!(cx, 0);
            let value = borrow_store!(cx, |store: &Merk| {
                store.get(key.as_slice())
            });

            let value = match value {
                Some(value) => value,
                None => panic!("no value found for key")
            };

            let buffer = cx.buffer(value.len() as u32)?;
            for i in 0..value.len() {
                let n = cx.number(value[i]);
                buffer.set(&mut cx, i as u32, n)?;
            }
            Ok(buffer.upcast())
        }

        method rootHash(mut cx) {
            let hash = borrow_store!(cx, |store: &Merk| -> Result<[u8; 20], failure::Error> {
                Ok(store.root_hash())
            });


            let buffer = cx.buffer(20)?;
            for i in 0..20 {
                let n = cx.number(hash[i]);
                buffer.set(&mut cx, i as u32, n)?;
            }
            Ok(buffer.upcast())
        }

        method batch(mut cx) {
            let args: Vec<Handle<JsMerk>> = vec![ cx.this() ];
            Ok(JsBatch::new(&mut cx, args)?.upcast())
        }

        method flushSync(mut cx) {
            borrow_store!(cx, |store: &mut Merk| store.flush());
            Ok(cx.undefined().upcast())
        }


        method clear(mut cx) {
            borrow_store!(cx, |store: &mut Merk| store.clear());
            Ok(cx.undefined().upcast())
        }

        method close(mut cx) {
            let rv = cx.undefined().upcast();
            let this = cx.this();
            let guard = cx.lock();
            let handle = this.borrow(&guard);
            let res = handle.store.lock();
            match res {
                Ok(mut store) => {
                    store.take();
                }
                _=>panic!("Failed to close store")
            }
            Ok(rv)
        }


        method commitSync(mut cx) {
            borrow_store!(cx, |store: &mut Merk| {
                store.commit(&[])
            });

            Ok(cx.undefined().upcast())
        }

        method proveSync(mut cx) {
            let upcasted_query = cx.argument::<JsArray>(0)?.to_vec(&mut cx)?;
            let mut query = Vec::with_capacity(upcasted_query.len());
            for value in upcasted_query {
                let buffer = value.downcast::<JsBuffer>().unwrap();
                let vec = cx.borrow(
                    &buffer,
                    |buffer| buffer.as_slice().to_vec()
                );
                query.push(vec);
            }

            let proof = borrow_store!(cx, |store: &Merk| {
                store.prove(query.as_slice())
            });

            let buffer = cx.buffer(proof.len() as u32)?;
            for i in 0..proof.len() {
                let n = cx.number(proof[i]);
                buffer.set(&mut cx, i as u32, n)?;
            }
            Ok(buffer.upcast())
        }
    }

    pub class JsBatch for Batch {
        init(mut cx) {
            let merk = cx.argument::<JsMerk>(0)?;
            let guard = cx.lock();
            let handle = merk.borrow(&guard);
            Ok(Batch {
                ops: Some(BTreeMap::new()),
                store: handle.store.clone()
            })
        }

        method put(mut cx) {
            let res = {
                let key = buffer_arg_to_vec!(cx, 0);
                let value = buffer_arg_to_vec!(cx, 1);
                // TODO: assert lengths

                let mut this = cx.this();
                let guard = cx.lock();
                let mut handle = this.borrow_mut(&guard);

                if let Some(ops) = &mut handle.ops {
                    ops.insert(key, Op::Put(value));
                    Ok(())
                } else {
                    Err("batch was already committed")
                }
            };

            match res {
                Ok(_) => Ok(cx.this().upcast()),
                Err(err) => cx.throw_error(err)
            }
        }

        method delete(mut cx) {
            let res = {
                let key = buffer_arg_to_vec!(cx, 0);
                // TODO: assert length

                let mut this = cx.this();
                let guard = cx.lock();
                let mut handle = this.borrow_mut(&guard);

                if let Some(ops) = &mut handle.ops {
                    ops.insert(key, Op::Delete);
                    Ok(())
                } else {
                    Err("batch was already committed")
                }
            };

            match res {
                Ok(_) => Ok(cx.this().upcast()),
                Err(err) => cx.throw_error(err)
            }
        }


        method applySync(mut cx) {
            let maybe_ops = {
                let mut this = cx.this();
                let guard = cx.lock();
                let mut handle = this.borrow_mut(&guard);
                handle.ops.take()
            };

            if let Some(ops) = maybe_ops {
                let mut batch = Vec::with_capacity(ops.len());
                for entry in ops {
                    batch.push(entry);
                }

                borrow_store!(cx, |store: &mut Merk| {
                    store.apply(batch.as_slice())
                });

                Ok(cx.undefined().upcast())
            } else {
                cx.throw_error("batch was already committed")
            }
        }

        method commit(mut cx) {
            cx.throw_error("not yet implemented")
        }
    }
}

fn verify_proof(mut cx: FunctionContext) -> JsResult<JsValue> {
    let proof_bytes: Vec<u8> = buffer_arg_to_vec!(cx, 0);
    let keys = cx.argument::<JsArray>(1)?.to_vec(&mut cx)?;
    let keys: Vec<Vec<u8>> = keys
        .iter()
        .map(|handle: &Handle<JsValue>| -> Handle<JsBuffer> {
            let res = handle.downcast::<JsBuffer>();
            match res {
                Ok(buffer) => buffer,
                Err(_err) => panic!("invalid proof key"),
            }
        })
        .map(|buffer| -> Vec<u8> {
            let guard = cx.lock();
            let buffer = buffer.borrow(&guard);
            buffer.as_slice().to_vec()
        })
        .collect();
    let expected_hash_bytes: Vec<u8> = buffer_arg_to_vec!(cx, 2);
    let mut expected_hash: merk::Hash = [0; 20];
    for i in 0..20 {
        let n = expected_hash_bytes[i];
        expected_hash[i] = n;
    }

    let res = merk::verify_proof(proof_bytes.as_slice(), keys.as_slice(), expected_hash);
    let js_result = match res {
        Ok(entries) => {
            let js_result = JsArray::new(&mut cx, entries.len() as u32);
            for (i, entry) in entries.iter().enumerate() {
                let value: Handle<JsValue> = match entry {
                    Some(value_bytes) => {
                        let buffer = cx.buffer(value_bytes.len() as u32).unwrap();
                        for j in 0..value_bytes.len() {
                            let n = cx.number(value_bytes[j]);
                            buffer.set(&mut cx, j as u32, n)?;
                        }

                        buffer.upcast()
                    }
                    None => cx.null().upcast(),
                };

                js_result.set(&mut cx, i as u32, value)?;
            }
            js_result.upcast()
        }
        Err(err) => panic!(err),
    };

    Ok(js_result)
}

register_module!(mut m, {
    m.export_class::<JsMerk>("Merk")?;
    m.export_function("verifyProof", verify_proof)
});

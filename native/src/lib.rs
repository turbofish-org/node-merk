extern crate failure;
extern crate merk;
extern crate neon;

use merk::chunks::ChunkProducer;
use merk::{proofs::Query, verify, Merk, Op};
use neon::prelude::*;
use std::collections::BTreeMap;
use std::ops::DerefMut;
use std::path::Path;
use std::rc::Rc;
use std::sync::Mutex;

pub struct MerkHandle {
    pub store: Rc<Mutex<Option<Merk>>>,
    chunk_producer: Rc<Mutex<Option<ChunkProducer<'static>>>>,
}

pub struct Batch {
    ops: Option<BTreeMap<Vec<u8>, Op>>,
    store: Rc<Mutex<Option<Merk>>>,
    chunk_producer: Rc<Mutex<Option<ChunkProducer<'static>>>>,
}

pub struct Restorer {
    restorer: Option<merk::restore::Restorer>,
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
            Err(err) => panic!("{}", err),
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
                    store: Rc::new(Mutex::new(Some(store))),
                    chunk_producer: Rc::new(Mutex::new(None)),
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

        method getChunkSync(mut cx) {
            let chunk_index = cx.argument::<JsNumber>(0)?.value() as usize;
            let chunk = use_chunk_producer(&mut cx, |cp|{
                cp.chunk(chunk_index).unwrap()
            });

            let buffer = cx.buffer(chunk.len() as u32)?;
            for i in 0..chunk.len() {
                let n = cx.number(chunk[i]);
                buffer.set(&mut cx, i as u32, n)?;
            }

            Ok(buffer.upcast())
        }

        method numChunks(mut cx) {
            let chunk_len =  use_chunk_producer(&mut cx, |cp| cp.len() as u32);
            Ok(cx.number(chunk_len).upcast())
        }

        method rootHash(mut cx) {
            let hash = borrow_store!(cx, |store: &Merk| -> Result<[u8; 32], failure::Error> {
                Ok(store.root_hash())
            });

            let buffer = cx.buffer(32)?;
            for i in 0..32 {
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

        method destroy(mut cx) {
            let rv = cx.undefined().upcast();
            let this = cx.this();
            let guard = cx.lock();
            let handle = this.borrow(&guard);
            let res = handle.store.lock();
            match res {
                Ok(mut store) => {
                    store.take().unwrap().destroy().unwrap();
                }
                _=>panic!("Failed to destroy store")
            }

            Ok(rv)
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

        method proveSync(mut cx) {
            let upcasted_query = cx.argument::<JsArray>(0)?.to_vec(&mut cx)?;
            // let mut query = Vec::with_capacity(upcasted_query.len());
            let mut query = Query::new();


            for value in upcasted_query {
                let buffer = value.downcast::<JsBuffer>().unwrap();
                let key = cx.borrow(
                    &buffer,
                    |buffer| buffer.as_slice().to_vec()
                );
                query.insert_key(key);
            }

            let proof = borrow_store!(cx, |store: &Merk| {
                store.prove(query)
            });

            let buffer = cx.buffer(proof.len() as u32)?;
            for i in 0..proof.len() {
                let n = cx.number(proof[i]);
                buffer.set(&mut cx, i as u32, n)?;
            }
            Ok(buffer.upcast())
        }

        method checkpointSync(mut cx) {
            let path = cx.argument::<JsString>(0)?;
            borrow_store!(cx, |store: &Merk|{
                store.checkpoint(Path::new(&path.value()))
            });

            Ok(cx.undefined().upcast())
        }


    }



    pub class JsBatch for Batch {
        init(mut cx) {
            let merk = cx.argument::<JsMerk>(0)?;
            let guard = cx.lock();
            let handle = merk.borrow(&guard);
            Ok(Batch {
                ops: Some(BTreeMap::new()),
                store: handle.store.clone(),
                chunk_producer: handle.chunk_producer.clone(),
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

        method commitSync(mut cx) {
            {
                let mut this = cx.this();
                let guard = cx.lock();
                let handle = this.borrow_mut(&guard);
                let mut lock = handle.chunk_producer.lock().unwrap();
                lock.deref_mut().take();
            }
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
                    store.apply(batch.as_slice(), &[])
                });

                Ok(cx.undefined().upcast())
            } else {
                cx.throw_error("batch was already committed")
            }
        }
    }

    pub class JsRestorer for Restorer {
        init(mut cx) {
            let path = cx.argument::<JsString>(0)?.value();
            let path = Path::new(&path);
            let mut expected_hash_bytes = [0; 32];

            for (k, v) in buffer_arg_to_vec!(cx, 1).iter().enumerate() {
                expected_hash_bytes[k] = *v;
            }
            let stated_length = cx.argument::<JsNumber>(2)?.value() as usize;
            let restorer = Some(merk::Merk::restore(path, expected_hash_bytes, stated_length).unwrap());
            Ok(Restorer { restorer })
        }

        method processChunkSync(mut cx) {
            {
                let chunk_bytes = buffer_arg_to_vec!(cx, 0);
                let mut this = cx.this();
                let guard = cx.lock();
                let mut handle = this.borrow_mut(&guard);
                handle.restorer.as_mut().expect("Restorer has already been finalized").process_chunk(chunk_bytes.as_slice()).unwrap();
            }
            Ok(cx.undefined().upcast())
        }

        method remainingChunks(mut cx) {
            let remaining_chunks = {
               let mut this = cx.this();
               let guard = cx.lock();
               let handle = this.borrow_mut(&guard);
               handle.restorer.as_ref().expect("Restorer has already been finalized").remaining_chunks()
            };
            match remaining_chunks {
                Some(n) => Ok(cx.number(n as u32).upcast()),
                None => Ok(cx.null().upcast())
            }
        }

        method finalizeSync(mut cx) {
            {
                let mut this = cx.this();
                let guard = cx.lock();
                let mut handle = this.borrow_mut(&guard);
                handle.restorer.take().expect("Restorer has already been finalized").finalize().unwrap();
            }

            Ok(cx.undefined().upcast())
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
    let mut expected_hash: merk::Hash = [0; 32];
    for i in 0..32 {
        let n = expected_hash_bytes[i];
        expected_hash[i] = n;
    }

    let entries = verify(proof_bytes.as_slice(), expected_hash).expect("Failed to parse proof");

    let js_result = JsArray::new(&mut cx, 0);
    for (i, key) in keys.iter().enumerate() {
        let entry = entries
            .get(key.as_slice())
            .expect(&format!("Failed to parse proof for key {:?}", key)[..]);
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

    Ok(js_result.upcast())
}

fn use_chunk_producer<'a, T, F: FnMut(&mut ChunkProducer) -> T>(
    cx: &'a mut CallContext<JsMerk>,
    mut op: F,
) -> T {
    let mut this = cx.this();
    let guard = cx.lock();

    let mut handle = this.borrow_mut(&guard);
    if handle.chunk_producer.lock().unwrap().as_ref().is_none() {
        let merk: &'static Merk = {
            let mut lock = handle.store.lock().unwrap();
            let store = lock.as_mut().unwrap();
            let ptr: *mut Merk = store;
            unsafe { std::mem::transmute(ptr) }
        };

        let cp = Rc::new(Mutex::new(Some(ChunkProducer::new(merk).unwrap())));

        handle.chunk_producer = cp;
    }

    let mut lock = handle.chunk_producer.lock().unwrap();
    let cp = lock.as_mut().unwrap();
    op(cp)
}

register_module!(mut m, {
    m.export_class::<JsMerk>("Merk")?;
    m.export_class::<JsRestorer>("Restorer")?;
    m.export_function("verifyProof", verify_proof)
});

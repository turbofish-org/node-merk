# node-merk

*Node.js bindings for [Merk](https://github.com/nomic-io/merk), a fast Merkle
tree library built on RocksDB.*

## Usage

`npm install merk`

```js
let { Merk, verifyProof, restore } = require('merk')

let keys = [Buffer.from('key1'), Buffer.from('key2')]
let values = [Buffer.from('value1'), Buffer.from('value2')]

// create or load store
let db = Merk('./state.db')

// write some values
db.batch()
  .put(keys[0], values[0])
  .put(keys[1], values[1])
  .commitSync()

// get a value
let value = db.getSync(keys[0])

// get the Merkle root
let hash = db.rootHash()

// create a merkle proof
let proof = db.proveSync(keys)

// verify a merkle proof
let proofResult = verifyProof(proof, keys, hash)

// close Merk and the underlying RocksDB instance
db.close()
```

## API
```js
let { Merk, verifyProof, restore } = require('merk')
```

### Database

#### `let db = Merk(path)`
Create or open an existing `Merk` by file path. `path` is a string.

#### `let value = db.getSync(key)`
Synchronously fetch a value from the db by its key. `key` is a buffer.

`value` is a buffer containing the value. This method throws if the the key isn't
found in the database.

#### `let hash = db.rootHash()`
Computes the Merkle root of the underlying tree. `hash` is a 20-byte buffer.

#### `let numChunks = db.numChunks()`
Returns the number of chunks required to prove the current tree.

#### `let chunk = db.getChunkSync(index)`
Returns the chunk proof for the given index.

`index` is a number and must be less than `db.numChunks()`. `chunk` is a buffer.

#### `db.flushSync()`
Forces RocksDB to flush data to disk. You probably shouldn't need to do this
manually.

#### `db.close()`
Gracefully shut down Merk and RocksDB before a process exit, say.

#### `db.destroy()`
Closes the database and removes all stored data from disk. Useful for cleaning
up checkpoints.

#### `db.checkpoint(path)`
Create a checkpoint at the desired path. A checkpoint is an immutable view of
the current tree state.

Creating a checkpoint is a cheap operation because, under the hood, RocksDB is
just creating some symlinks. You may open the checkpoint at this path as a
full-blown Merk: `let db2 = Merk(checkpointPath)`.

`path` is a string, and this method will throw if
anything already exists at this path.

### Batch
Batches accumulate `put` and `delete` operations to be atomically committed to
the database.

#### `let batch = db.batch()`
Creates an empty batch, which can be used to build an atomic set of writes and
deletes.

#### `batch.put(key, value)`
Adds a `put` operation to the batch. `key` and `value` are buffers. The write is
executed after the batch is committed.

Returns the batch instance, so you may chain multiple `.put()` or `.delete()` calls.

#### `batch.delete(key)`
Adds a `delete` operation to the batch, removing the database entry for the key.
`key` is a buffer. Deletes happen when the batch is committed.

#### `batch.commitSync()`
Synchronously execute all `put` and `delete` operations in this batch, writing
them to the database.

### Proofs

#### `let proof = db.proveSync(keys)`
Generate a merkle proof for some keys. `keys` is an array of buffers.

The returned `proof` is a buffer, which can be verified against an expected root
hash to retrieve the proven values.

#### `let result = verifyProof(proof, keys, hash)`
Verify a Merkle proof against an expected root hash. `proof` is a buffer, `keys`
is an array of buffers, and `hash` is a buffer.

The returned `result` is an array of buffers corresponding to values for each key,
in the same order as the provided keys.

Throws if any of the provided keys can't be proven.

### Restorer
The `restore()` function can be used to reconstruct a `Merk` from chunk proofs.

See the `State Sync` section below for more details on how to integrate this
with a state machine replication engine like Tendermint.

#### `let restorer = restore(path, hash, numChunks)`
Create and returns a `Restorer` instance, which can sequentially process chunks to
reconstruct a `Merk`. 

`path` is a string where you'd like to create the database. Throws if anything exists at this path already.

`hash` is the expected root hash of the tree you're restoring.

`numChunks` is a number, how many chunks you're expecting to process. Throws if
you process more than this many chunks.


#### `restorer.processChunkSync(chunk)`
Process a chunk proof, as returned from `db.getChunkSync()`. `chunk` is a
buffer.

Chunks must be processed in-order. Read more in the `State Sync` section below.

Throws if the chunk isn't valid.

#### `restorer.finalizeSync()`
Synchronously finalizes the restored Merk, which can then be opened safely with
the `Merk` constructor.

Must be called after all chunks have been processed.

## State Sync

If you're using Merk, there's a good chance you're also using Tendermint. This
section will give you a quick overview on how the pieces fit together to
accomplish state sync with Tendermint and Merk.

### Keep some historical snapshots
First you'll need to decide on a strategy for when to create snapshots (and
their associated Merk checkpoints). Maybe each block, maybe every 100 blocks, etc.

To create a snapshot, you'll do something like this:
```javascript
let snapshotMeta = {}
let height = 200
let checkpointPath = `./path/to/merk-${height}.db`
db.checkpoint(checkpointPath)

snapshotMeta[height] = {
  checkpointPath,
  hash: db.rootHash(),
  chunks: db.numChunks(),
}
```

You'll need to refer back to this `snapshotMeta` object in a few of the following ABCI messages.

### `ListSnapshots`
[ListSnapshots](https://docs.tendermint.com/master/spec/abci/abci.html#listsnapshots)

When you receive the `ListSnapshots` ABCI message, send back an array of the
`snapshotMeta` objects from the above section, plus other data for the
[Snapshot](https://docs.tendermint.com/master/spec/abci/abci.html#snapshot) data
fields for `metadata` and `format`, which will depend on your application.

### `LoadSnapshotChunk`
[LoadSnapshotChunk](https://docs.tendermint.com/master/spec/abci/abci.html#loadsnapshotchunk)

Tendermint is requesting a specific chunk from a snapshot you listed in your
`ListSnapshots` response.

Load the checkpoint and fetch the requested chunk like this:
```javascript
let { Merk } = require('merk')
// `chunk` is the index of the requested chunk within the snapshot for this height
let { height, chunk } = loadSnapshotChunkRequest

let snapshot = snapshotMeta[height]
let db = Merk(snapshot.checkpointPath)

// Send this back to Tendermint:
let loadSnapshotChunkResponse = {
  chunk: db.getChunkSync(chunk)
}
```

### `OfferSnapshot`
[OfferSnapshot](https://docs.tendermint.com/master/spec/abci/abci.html#offersnapshot)

If you're a newly-connected node, Tendermint will use this ABCI message to send
you metadata about available snapshots. It's up to you to decide which of the
recently-offered snapshots you'd like to accept.

When you accept a snapshot, save accepted snapshot metadata somewhere for use
in the next ABCI message handler:

```javascript
let { height, chunks } = offerSnapshotRequest.snapshot
let { appHash } = offerSnapshotRequest
let acceptedSnapshot = { height, chunks, appHash }
```


No Merk interaction required here.

### `ApplySnapshotChunk`
[ApplySnapshotChunk](https://docs.tendermint.com/master/spec/abci/abci.html#applysnapshotchunk)

You're a syncing node, and you've accepted a snapshot. Here's where you receive
a chunk and use it to partially-restore a Merk.

Tendermint applies chunks sequentially, so for the first chunk, you'll create a
restorer. For all chunks, you'll process the chunk with this restorer. Then
you'll finalize the restorer after the last chunk:

```javascript
let { restore } = require('merk')
let { index, chunk } = applySnapshotChunkRequest
let { height, chunks, appHash } = acceptedSnapshot

let path = 'path/to/my.db'
// Just for the first chunk:
let restorer = restore(path, appHash, chunks)

// For all chunks:
restorer.processChunkSync(chunk)

// Just for the last chunk
if(index === chunks - 1) {
  restorer.finalizeSync()
}

// Now the restored Merk may be safely opened and state sync is complete!
let db = Merk(path)
```







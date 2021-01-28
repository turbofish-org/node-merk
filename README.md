# node-merk

*Node.js bindings for [Merk](https://github.com/nomic-io/merk)*

## Usage

`npm install merk`

```js
let { Merk, verifyProof } = require('merk')

// create or load store
let db = Merk('./state.db')

// modify values
db.batch()
  .put(Buffer.from('key1'), Buffer.from('value1'))
  .put(Buffer.from('key2'), Buffer.from('value2'))
  .applySync()

db.batch().delete(Buffer.from('key1')).applySync()
// for now, after applying changes, you must commit before computing root hash / making proofs
db.commitSync()

// get value
let value = db.getSync(Buffer.from('key2'))

// get Merkle root
let hash = db.rootHash()

// create merkle proof
let keys = [
  Buffer.from('key1'),
  Buffer.from('key2')
]
let proof = db.proveSync(keys)

// verify a merkle proof
let proofResult = verifyProof(proof, keys, db.rootHash())
console.log(proofResult)

```

# node-merk

*Node.js bindings for [Merk](https://github.com/nomic-io/merk)*

## Usage

`npm install merk`

```js
let merk = require('merk')

// create or load store
let db = merk('./state.db')

// get value
let value = db.getSync(Buffer.from('mykey'))

// get Merkle root
let hash = db.rootHash()

// create merkle proof
let proof = db.proveSync([
  Buffer.from('key1'),
  Buffer.from('key2')
])

// modify values
db.batch()
  .put(Buffer.from('key1'), Buffer.from('value1'))
  .put(Buffer.from('key2'), Buffer.from('value2'))
  .delete(Buffer.from('key3'))
  .commitSync()
```

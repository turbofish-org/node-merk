let test = require('ava')
let { Merk, verifyProof, restore } = require('../lib/index.js')

function key(i) {
  return Buffer.from(`key${i}`)
}

function value(i) {
  return Buffer.from(`value${i}`)
}

function makeTree(nodes) {
  let db = Merk(`./${nodes}-nodes.db`)

  let indices = []
  for (let i = 0; i < nodes; i++) {
    indices.splice(Math.floor(Math.random() * indices.length), 0, i)
  }

  indices.forEach((i) => {
    db.batch().put(key(i), value(i)).commitSync()
  })

  return db
}

function assertTree(t, size, tree) {
  for (let i = 0; i < size; i++) {
    t.is(tree.getSync(key(i)).toString(), value(i).toString())
  }
}

function assertProofs(t, dbs, size) {
  let k = Math.floor(Math.random() * size)
  let proofs = dbs.map((db) => {
    return db.proveSync([key(k)])
  })
  t.is(proofs.length, 3)
  proofs.forEach((proof) => {
    t.is(
      verifyProof(proof, [key(k)], dbs[0].rootHash())[0].toString(),
      value(k).toString()
    )
  })
}

function syncTree(tree, path) {
  let restorer = restore(path, tree.rootHash(), tree.numChunks())
  for (let i = 0; i < tree.numChunks(); i++) {
    let chunk = tree.getChunkSync(i)
    restorer.processChunkSync(chunk)
  }
  restorer.finalizeSync()

  return Merk(path)
}

function checkpointTree(tree, path) {
  tree.checkpointSync(path)
  let checkpoint = Merk(path)
  return checkpoint
}

function testSize(size) {
  test(`${size}-node tree`, (t) => {
    let db = makeTree(size)
    assertTree(t, size, db)
    let db2 = checkpointTree(db, `./${size}-nodes-checkpoint.db`)
    assertTree(t, size, db2)
    let db3 = syncTree(db2, `./${size}-nodes-restored.db`)
    assertTree(t, size, db3)

    assertProofs(t, [db, db2, db3], size)
    db.destroy()
    db2.destroy()
    db3.destroy()
    t.pass()
  })
}

for (let size = 1; size < 100; size++) {
  testSize(size)
}

for (let size = 10000; size < 10011; size++) {
  testSize(size)
}

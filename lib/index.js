'use strict'

const { Merk, verifyProof } = require('../native')

module.exports.Merk = function createMerk(path) {
  let merk = new Merk(path)
  let { proveSync } = merk
  merk.proveSync = function (keys) {
    let sortedKeys = keys.slice().sort((a, b) => Buffer.compare(a, b))
    return proveSync.call(merk, sortedKeys)
  }
  return merk
}
module.exports.verifyProof = function verify(proof, keys, expectedHash) {
  let keyIndices = Object.keys(keys).map((originalIndex) => {
    return {
      originalIndex,
    }
  })
  keyIndices.sort((a, b) =>
    Buffer.compare(keys[a.originalIndex], keys[b.originalIndex])
  )
  keyIndices.forEach((keyIndex, newIndex) => {
    keyIndex.newIndex = newIndex
  })
  let sortedKeys = keyIndices.map((keyIndex) => keys[keyIndex.originalIndex])
  let proofResult = verifyProof(proof, sortedKeys, expectedHash)
  let result = []
  for (let i = 0; i < proofResult.length; i++) {
    result[keyIndices[i].originalIndex] = proofResult[keyIndices[i].newIndex]
  }
  return result
}

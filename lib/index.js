'ue strict'

const { Merk, verifyProof } = require('../native')

module.exports.Merk = function createMerk(path) {
  let merk = new Merk(path)
  process.on('exit', () => merk.flushSync())
  return merk
}
module.exports.verifyProof = verifyProof

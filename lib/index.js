'ue strict'

const { Merk } = require('../native')

module.exports = function createMerk (path) {
  let merk = new Merk(path)
  process.on('exit', () => merk.flushSync())
  return merk
}

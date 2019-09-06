#!/usr/bin/env node

const { spawn } = require('child_process')
const { accessSync, symlinkSync } = require('fs')
const { join } = require('path')

try {
  let destPath = './native/index.node'
  let sourcePath = join('./build', binaryName())

  try {
    accessSync(destPath)
    // already installed
    process.exit(0)
  } catch (err) {}

  accessSync(sourcePath)
  symlinkSync(join('..', sourcePath), destPath)
  console.log(`Using prebuilt binary (${sourcePath})`)
} catch (err) {
  console.log(`No prebuilt binary, falling back to build`)
  spawn('npm', [ 'run', 'build' ], { stdio: 'inherit' })
}

function binaryName () {
  if (process.arch !== 'x64') {
    throw Error(`Unsupported architecture: ${process.arch}`)
  }

  if (process.platform === 'darwin') {
    return 'x86_64-apple-darwin.node'
  } else if (process.platform === 'linux') {
    return 'x86_64-unknown-linux-gnu.node'
  } else {
    throw Error(`Unsupported platform: ${process.platform}`)
  }
}

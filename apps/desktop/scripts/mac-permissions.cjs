// What macOS may ask a person for on Locust's behalf, and why (the first Mac run, 2026-10-30).
//
// Electron's own Info.plist carries a reason for the camera, Bluetooth, the
// microphone and system audio capture, each "This app needs access to ...".
// Locust uses one of them: the microphone, for voice typing and voice calls
// (its reason is set in electron-builder.yml, mac.extendInfo). Nothing in
// Locust asks for the camera, Bluetooth or system audio, so those reasons are
// taken out: a Mac then never shows a prompt Locust cannot explain.
// adhoc-sign.cjs calls this before it signs, so the signature covers the result.

const { execFileSync } = require('node:child_process')

const UNUSED = [
  'NSCameraUsageDescription',
  'NSBluetoothAlwaysUsageDescription',
  'NSBluetoothPeripheralUsageDescription',
  'NSAudioCaptureUsageDescription'
]

/** Take the reasons Locust has no use for out of an app's Info.plist. */
function trimUnusedPermissions(plist) {
  for (const key of UNUSED) {
    try {
      execFileSync('plutil', ['-remove', key, plist], { stdio: 'ignore' })
    } catch {
      // Not there: nothing to take out.
    }
  }
}

module.exports = { UNUSED, trimUnusedPermissions }

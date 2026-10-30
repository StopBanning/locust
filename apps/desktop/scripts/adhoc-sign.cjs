// electron-builder `afterSign` hook: sign the macOS app AD HOC.
//
// The Mac build is not signed with a Developer ID yet (that needs an Apple
// Developer account), so electron-builder's own signing is off
// (`mac.identity: null`). An app with NO signature, downloaded onto an Apple
// silicon Mac, is reported as "damaged and can't be opened"; one signed ad hoc
// (`codesign --sign -`) is asked about once, and System Settings > Privacy &
// Security > Open Anyway lets it run. This runs after the (skipped) signing and
// before the .dmg is made, so the image carries the signed app.
//
// Windows and Linux builds pass straight through.

const { execFileSync } = require('node:child_process')
const { join } = require('node:path')

const { trimUnusedPermissions } = require('./mac-permissions.cjs')

exports.default = async function adhocSign(context) {
  if (context.electronPlatformName !== 'darwin') return
  const app = join(context.appOutDir, `${context.packager.appInfo.productFilename}.app`)
  // Before signing: the signature seals Info.plist as it is then.
  trimUnusedPermissions(join(app, 'Contents', 'Info.plist'))
  execFileSync('codesign', ['--force', '--deep', '--sign', '-', app], { stdio: 'inherit' })
  execFileSync('codesign', ['--verify', '--deep', '--strict', '--verbose=2', app], { stdio: 'inherit' })
}

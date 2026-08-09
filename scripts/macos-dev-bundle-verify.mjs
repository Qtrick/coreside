#!/usr/bin/env node
/**
 * Inspect the prepared macOS adaptive development .app.
 * Does not launch the app.
 */
import {
  DEV_APP,
  printIdentityReport,
  verifyDevBundleStructure,
} from "./lib/macos-dev-bundle.mjs";

const result = verifyDevBundleStructure(DEV_APP);
printIdentityReport({
  appPath: result.appPath,
  executablePath: result.executablePath,
  label: "prepared adaptive development bundle",
});
if (!result.ok) {
  console.error(`FAIL: ${result.errors.join("; ")}`);
  process.exit(1);
}
console.log("PASS development bundle structure");

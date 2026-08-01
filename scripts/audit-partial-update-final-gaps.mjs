#!/usr/bin/env node
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const audit = JSON.parse(
  fs.readFileSync(path.join(root, "reports/partial-update-final-gap-audit.json"), "utf8"),
);
if (!audit.systems?.length) process.exit(1);
console.log("gap systems", audit.systems.length);

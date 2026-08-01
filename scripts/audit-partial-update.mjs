#!/usr/bin/env node
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const matrix = JSON.parse(
  fs.readFileSync(path.join(root, "reports/partial-update-feature-matrix.json"), "utf8"),
);
if (!matrix.count) process.exit(1);
console.log("features", matrix.count);

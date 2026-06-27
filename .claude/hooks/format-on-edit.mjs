#!/usr/bin/env node
// PostToolUse hook: auto-format the file just edited or written.
// Reads the hook event JSON from stdin and dispatches based on file extension.
// Non-blocking: any failure here must not interrupt the agent's work.
//
// Mitatete stack: Rust (src-tauri/) + vanilla HTML/CSS/JS (src/).
//   - .rs              -> rustfmt
//   - .js/.css/.html/.json/.md -> prettier (if available)

import { readFileSync, existsSync } from "node:fs";
import { execFileSync } from "node:child_process";
import path from "node:path";

function run(cmd, args) {
  try {
    execFileSync(cmd, args, { stdio: "ignore" });
  } catch {
    // ignore: formatting failures must not block the agent
  }
}

let event;
try {
  event = JSON.parse(readFileSync(0, "utf-8"));
} catch {
  process.exit(0);
}

const filePath = event?.tool_input?.file_path;
if (!filePath || !existsSync(filePath)) {
  process.exit(0);
}

const ext = path.extname(filePath);

if (ext === ".rs") {
  // rustfmt formats a single file in place; edition kept in sync with Cargo.toml
  run("rustfmt", ["--edition", "2021", filePath]);
} else if (
  [".js", ".mjs", ".cjs", ".css", ".html", ".json", ".md"].includes(ext)
) {
  run("npx", ["prettier", "--write", filePath]);
}

process.exit(0);

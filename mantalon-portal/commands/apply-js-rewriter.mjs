#!/usr/bin/env node
/**
 * CLI to run applyJsProxyOnJs on an input file for testing.
 *
 * Usage:
 *   npm run apply-js-proxy -- --input path/to/input.js --output path/to/out.js
 *   npm run apply-js-proxy -- -i path/to/input.js        # prints to stdout
 *
 * (The `--` after npm run is required to pass args through npm.)
 */

import fs from "fs/promises";
import path from "path";
import { applyJsProxyOnJs } from "../dist/js-rewriter.js";

function parseArgs(argv) {
  const opts = {};
  for (let i = 0; i < argv.length; i++) {
    const a = argv[i];
    if (a === "--input" || a === "-i") {
      opts.input = argv[++i];
    } else if (a === "--output" || a === "-o") {
      opts.output = argv[++i];
    } else if (a === "--help" || a === "-h") {
      opts.help = true;
    }
  }
  return opts;
}

async function main() {
  const rawArgs = process.argv.slice(2);
  const opts = parseArgs(rawArgs);

  if (opts.help || !opts.input) {
    console.log(`
Usage:
  npm run apply-js-proxy -- --input path/to/file.js [--output path/to/out.js]

Options:
  --input, -i   Input file path (required)
  --output, -o  Output file path. If omitted, result is printed to stdout.
  --help, -h    Show this help
`);
    process.exit(opts.input ? 0 : 1);
  }

  const inputPath = path.resolve(process.cwd(), opts.input);
  let src;
  try {
    src = await fs.readFile(inputPath, "utf8");
  } catch (err) {
    console.error(`Error reading input file "${inputPath}":`, err);
    process.exitCode = 2;
    return;
  }

  let transformed;
  try {
    transformed = applyJsProxyOnJs(src);
  } catch (err) {
    console.error("Error running applyJsProxyOnJs:", err);
    process.exitCode = 3;
    return;
  }

  if (opts.output) {
    const outPath = path.resolve(process.cwd(), opts.output);
    try {
      await fs.mkdir(path.dirname(outPath), { recursive: true });
      await fs.writeFile(outPath, transformed, "utf8");
      console.log(`Wrote transformed output to ${outPath}`);
    } catch (err) {
      console.error(`Error writing output file "${opts.output}":`, err);
      process.exitCode = 4;
    }
  } else {
    process.stdout.write(transformed);
  }
}

main();

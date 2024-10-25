import esbuild from "esbuild";
import { polyfillNode } from "esbuild-plugin-polyfill-node";

esbuild.build({
  entryPoints: ["./src/sw.ts"],
  bundle: true,
  outfile: "./sw_bundle.js",
  format: "iife",
  platform: "browser",
  plugins: [
    polyfillNode({}),
  ],
  minify: true,
}).then(() => {
  console.log("Service worker bundled successfully.");
}).catch(() => process.exit(1));

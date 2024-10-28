import esbuild from "esbuild";

esbuild.build({
  entryPoints: ["./js-proxy/main.ts"],
  bundle: true,
  outfile: "./js-proxy-bundle.js",
  format: "iife",
  platform: "browser",
  minify: true,
}).then(() => {
  console.log("JS proxy script bundled successfully.");
}).catch(() => process.exit(1));
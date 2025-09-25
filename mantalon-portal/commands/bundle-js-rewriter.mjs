import esbuild from "esbuild";

esbuild.build({
  entryPoints: ["./src/js-rewriter.ts"],
  bundle: true,               // only bundle your code
  outfile: "./dist/js-rewriter.js",
  platform: "node",            // Node target
  format: "cjs",               // CommonJS avoids dynamic require issues
  minify: false,
  sourcemap: true,
  external: ["fs", "path", "recast", "tough-cookie"], // don't bundle Node libs
}).then(() => console.log("js-proxy bundled for Node successfully."))
  .catch((err) => {
    console.error(err);
    process.exit(1);
  });

import esbuild from 'esbuild';

esbuild.build({
  entryPoints: ['./src/sw.ts'],
  bundle: true,
  outfile: './sw_bundle.js',
  format: 'iife',
  platform: 'browser',
  minify: true,
}).then(() => {
  console.log('Service worker bundled successfully.');
}).catch(() => process.exit(1));

import esbuild from 'esbuild';

esbuild.build({
  entryPoints: ['./sw.js'],
  bundle: true,
  outfile: './sw_bundle.js',
  format: 'iife',
  platform: 'browser',
  minify: true,
}).then(() => {
  console.log('Service worker bundled successfully.');
}).catch(() => process.exit(1));

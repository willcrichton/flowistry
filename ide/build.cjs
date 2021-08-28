const esbuild = require("esbuild");
const { sassPlugin } = require("esbuild-sass-plugin");
const { cli } = require("@wcrichto/esbuild-utils");

const options = cli();

let common = {
  outdir: "out",
  external: ["vscode"],
  plugins: [sassPlugin()],
  bundle: true,
  sourcemap: true,
  ...options
};

let extension = esbuild.build({
  entryPoints: ["src/extension.ts"],
  platform: "node",
  ...common
});

let page = esbuild.build({
  entryPoints: ["src/effects_page.tsx"],
  ...common
});

Promise.all([extension, page])
  .then(() => console.log("Build complete."))
  .catch(() => process.exit(1));

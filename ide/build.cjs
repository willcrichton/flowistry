const estrella = require("estrella");
const { sassPlugin } = require("esbuild-sass-plugin");
const { cli } = require("@wcrichto/esbuild-utils");
const toml = require("toml");
const fs = require("fs");
const pkg = require("./package.json");

const options = cli();

const rust_toolchain = toml.parse(fs.readFileSync("../rust-toolchain.toml"));
const define = {
  TOOLCHAIN: JSON.stringify(rust_toolchain.toolchain),
  VERSION: JSON.stringify(pkg.version),
};

let common = {
  outdir: "out",
  external: ["vscode"],
  plugins: [sassPlugin()],
  bundle: true,
  sourcemap: true,
  tslint: true,
  define,
  ...options
};

let extension = estrella.build({
  entryPoints: ["src/extension.ts", "src/tests/unit/util/slice_helpers.ts"],
  platform: "node",
  ...common
});

let page = estrella.build({
  entryPoints: ["src/effects_page.tsx"],
  ...common
});

Promise.all([extension, page])
  .then(() => console.log("Build complete."))
  .catch(() => process.exit(1));

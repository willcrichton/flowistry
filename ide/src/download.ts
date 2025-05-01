import AdmZip from "adm-zip";
import * as cp from "child_process";
import { promises as fs } from "fs";
import got from "got";
import _ from "lodash";
import path from "path";
import * as util from "util";

import { globals } from "./extension";
import { log } from "./logging";
import { cargo_bin } from "./setup";

declare const VERSION: string;

export let download = async () => {
  // globals.status_bar.set_state("loading", "Downloading Flowistry");
  // let exec = async (cmd: string) => {
  //   let exec = util.promisify(cp.exec);
  //   let { stdout } = await exec(cmd);
  //   return stdout.trim();
  // };

  // let rustc_info = await exec("rustc -Vv");
  // let [__, target] = _.chain(rustc_info)
  //   .split("\n")
  //   .map((line) => line.split(": "))
  //   .find(([key]) => key === "host")
  //   .value();

  // let repo = "willcrichton/flowistry";
  // let release_base_url = `https://github.com/${repo}/releases/download/v${VERSION}`;
  // let release_name = `${target}.zip`;
  // let release_url = `${release_base_url}/${release_name}`;

  // let dir = cargo_bin();
  // log(`Downloading ${release_url} to ${dir}`);
  // let buffer = await got.get(release_url).buffer();
  // let zip = new AdmZip(buffer);
  // zip.extractAllTo(dir, true);

  // // Ensure downloaded binaries are executable
  // let suffix = process.platform == "win32" ? ".exe" : "";
  // await fs.chmod(path.join(dir, "cargo-flowistry" + suffix), "755");
  // await fs.chmod(path.join(dir, "flowistry-driver" + suffix), "755");

  globals.status_bar.set_state("idle");
};

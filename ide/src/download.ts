import AdmZip from "adm-zip";
import * as cp from "child_process";
import got from "got";
import _ from "lodash";
import os from "os";
import path from "path";
import * as util from "util";

import { log } from "./logging";

declare const VERSION: string;

export let download = async () => {
  let exec = async (cmd: string) => {
    let exec = util.promisify(cp.exec);
    let { stdout } = await exec(cmd);
    return stdout.trim();
  };

  let rustc_info = await exec("rustc -Vv");
  let [__, target] = _.chain(rustc_info)
    .split("\n")
    .map((line) => line.split(": "))
    .find(([key]) => key === "host")
    .value();

  let repo = "willcrichton/flowistry";
  let release_base_url = `https://github.com/${repo}/releases/download/v${VERSION}`;
  let release_name = `${target}.zip`;
  let release_url = `${release_base_url}/${release_name}`;

  let cargo_home = process.env.CARGO_HOME || path.join(os.homedir(), ".cargo");
  let cargo_bin = path.join(cargo_home, "bin");

  log(`Downloading ${release_url} to ${cargo_bin}`);
  let buffer = await got.get(release_url).buffer();
  let zip = new AdmZip(buffer);
  zip.extractAllTo(cargo_bin);
};

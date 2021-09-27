import * as cp from "child_process";
import _ from "lodash";
import * as util from "util";
import got from "got";
import os from "os";
import path from "path";
import stream from "stream";
import tar from "tar-fs";
import gunzip from "gunzip-maybe";

import {log} from "./vsc_utils";

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
    .find(([key]) => key == "host")
    .value();

  let repo = "willcrichton/flowistry";
  let release_base_url = `https://github.com/${repo}/releases/download/v${VERSION}`;
  let release_name = `${target}.tar.gz`;
  let release_url = `${release_base_url}/${release_name}`;

  let cargo_home = process.env.CARGO_HOME || path.join(os.homedir(), ".cargo");
  let cargo_bin = path.join(cargo_home, "bin");

  log("Downloading: ", release_url);
  let pipeline = util.promisify(stream.pipeline);
  await pipeline(got.stream(release_url), gunzip(10), tar.extract(cargo_bin));
};

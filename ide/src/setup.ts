import * as cp from "child_process";
import fs from "fs";
import _ from "lodash";
import open from "open";
import os from "os";
import path from "path";
import { Readable } from "stream";
import * as vscode from "vscode";

import { download } from "./download";
import { FlowistryError, FlowistryResult } from "./errors";
import { globals } from "./extension";
import { log } from "./logging";

declare const VERSION: string;
declare const TOOLCHAIN: {
  channel: string;
  components: string[];
};

// serde-compatible type
type Result<T> = { Ok: T } | { Err: FlowistryError };

/* eslint no-undef: "off" */
const LIBRARY_PATHS: Partial<Record<NodeJS.Platform, string>> = {
  darwin: "DYLD_LIBRARY_PATH",
  win32: "LIB",
};

export const get_flowistry_opts = async (cwd: string) => {
  const rustc_path = await exec_notify(
    "rustup",
    ["which", "--toolchain", TOOLCHAIN.channel, "rustc"],
    "Waiting for rustc..."
  );
  const target_info = await exec_notify(
    rustc_path,
    ["--print", "target-libdir", "--print", "sysroot"],
    "Waiting for rustc..."
  );

  const [target_libdir, sysroot] = target_info.split("\n");
  log("Target libdir:", target_libdir);
  log("Sysroot: ", sysroot);

  const library_path = LIBRARY_PATHS[process.platform] || "LD_LIBRARY_PATH";

  const PATH = cargo_bin() + ";" + process.env.PATH;

  return {
    cwd,
    [library_path]: target_libdir,
    SYSROOT: sysroot,
    RUST_BACKTRACE: "1",
    PATH,
  };
};

export let exec_notify = async (
  cmd: string,
  args: string[],
  title: string,
  opts?: any
): Promise<string> => {
  log("Running command: ", [cmd, ...args].join(" "));

  let proc = cp.spawn(cmd, args, opts);

  let read_stream = (stream: Readable): (() => string) => {
    let buffer: string[] = [];
    stream.setEncoding("utf8");
    stream.on("data", (data) => {
      log(data.toString().trimEnd());
      buffer.push(data.toString());
    });
    return () => buffer.join("").trim();
  };

  let stdout = read_stream(proc.stdout);
  let stderr = read_stream(proc.stderr);

  globals.status_bar.set_state("loading", title);

  return new Promise<string>((resolve, reject) => {
    proc.addListener("close", (_) => {
      globals.status_bar.set_state("idle");
      if (proc.exitCode !== 0) {
        reject(stderr());
      } else {
        resolve(stdout());
      }
    });
    proc.addListener("error", (e) => {
      globals.status_bar.set_state("idle");
      reject(e.toString());
    });
  });
};

export type CallFlowistry = <T>(
  _args: string[],
  _no_output?: boolean
) => Promise<FlowistryResult<T>>;

export let cargo_bin = () => {
  let cargo_home = process.env.CARGO_HOME || path.join(os.homedir(), ".cargo");
  return path.join(cargo_home, "bin");
};

export let cargo_command = (): [string, string[]] => {
  let cargo = "cargo";
  let toolchain = `+${TOOLCHAIN.channel}`;
  return [cargo, [toolchain]];
};

let findWorkspaceRoot = (): string | null => {
  let folders = vscode.workspace.workspaceFolders;
  if (!folders || folders.length === 0) {
    return null;
  }

  let hasCargoToml = (dir: string) =>
    fs.existsSync(path.join(dir, "Cargo.toml"));

  let activeEditor = vscode.window.activeTextEditor;
  if (!activeEditor) return null;

  let folderPath = folders[0].uri.fsPath;
  let activeFilePath = activeEditor.document.fileName;
  let components = path.relative(folderPath, activeFilePath).split(path.sep);
  let folderSubdirTil = (idx: number) =>
    path.join(folderPath, ...components.slice(0, idx));
  let idx = _.range(components.length).find((idx) =>
    hasCargoToml(folderSubdirTil(idx))
  );
  if (idx === undefined) return null;

  return folderSubdirTil(idx);
};

export async function setup(
  context: vscode.ExtensionContext
): Promise<CallFlowistry | null> {
  let workspace_root = findWorkspaceRoot();
  if (!workspace_root) return null;
  log("Workspace root", workspace_root);

  let [cargo, cargo_args] = cargo_command();

  let version;
  try {
    version = await exec_notify(
      cargo,
      [...cargo_args, "flowistry", "-V"],
      "Waiting for Flowistry...",
      { cwd: workspace_root }
    );
  } catch (e) {
    version = "";
  }

  if (version !== VERSION) {
    log(
      `Flowistry binary version ${version} does not match expected IDE version ${VERSION}`
    );
    let components = TOOLCHAIN.components.map((c) => ["-c", c]).flat();
    try {
      await exec_notify(
        "rustup",
        [
          "toolchain",
          "install",
          TOOLCHAIN.channel,
          "--profile",
          "minimal",
          ...components,
        ],
        "Installing nightly Rust..."
      );
    } catch (e: any) {
      let choice = await vscode.window.showErrorMessage(
        'Flowistry failed to install because rustup failed. Click "Show fix" to resolve, or click "Dismiss to attempt installation later.',
        "Show fix",
        "Dismiss"
      );

      if (choice === "Show fix") {
        open(
          "https://github.com/willcrichton/flowistry/blob/master/README.md#rustup-fails-on-installation"
        );
        await vscode.window.showInformationMessage(
          'Click "Continue" once you have completed the fix.',
          "Continue"
        );
      } else {
        return null;
      }
    }

    try {
      await download();
    } catch (e: any) {
      log("Install script failed with error:", e.toString());

      await exec_notify(
        cargo,
        [
          ...cargo_args,
          "install",
          "flowistry_ide",
          "--version",
          VERSION,
          "--force",
        ],
        "Flowistry binaries not available, instead installing Flowistry crate from source... (this may take a minute)"
      );
    }

    if (version === "") {
      vscode.window.showInformationMessage(
        "Flowistry has successfully installed!"
      );
    }
  }

  let flowistry_opts = await get_flowistry_opts(workspace_root);
  return async <T>(args: string[], no_output: boolean = false) => {
    let output;
    try {
      let editor = vscode.window.activeTextEditor;
      if (editor) {
        await editor.document.save();
      }

      output = await exec_notify(
        cargo,
        [...cargo_args, "flowistry", ...args],
        "Waiting for Flowistry...",
        flowistry_opts
      );
    } catch (e: any) {
      context.workspaceState.update("err_log", e);

      return {
        type: "BuildError",
        error: e,
      };
    }

    if (no_output) {
      return {
        type: "output",
        value: undefined as any,
      };
    }

    let output_typed: Result<T> = JSON.parse(output);
    if ("Err" in output_typed) {
      return output_typed.Err;
    } else {
      return {
        type: "output",
        value: output_typed.Ok,
      };
    }
  };
}

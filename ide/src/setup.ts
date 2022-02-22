import * as cp from "child_process";
import _ from "lodash";
import open from "open";
import { Readable } from "stream";
import * as vscode from "vscode";

import { download } from "./download";
import { FlowistryResult } from "./errors";
import { globals } from "./extension";
import { log } from "./logging";

declare const VERSION: string;
declare const TOOLCHAIN: {
  channel: string;
  components: string[];
};

// rustc_serialize-compatible types
interface Ok<T> {
  variant: "Ok";
  fields: [T];
}
interface Err {
  variant: "Err";
  fields: [string];
}
type Result<T> = Ok<T> | Err;

/* eslint no-undef: "off" */
const LIBRARY_PATHS: Partial<Record<NodeJS.Platform, string>> = {
  darwin: "DYLD_LIBRARY_PATH",
  win32: "LIB",
};

export const flowistry_cmd = `cargo +${TOOLCHAIN.channel} flowistry`;

export const get_flowistry_opts = async (cwd: string) => {
  const rustc_path = await exec_notify(
    `rustup which --toolchain ${TOOLCHAIN.channel} rustc`,
    "Waiting for rustc..."
  );
  const target_info = await exec_notify(
    `${rustc_path} --print target-libdir --print sysroot`,
    "Waiting for rustc..."
  );

  const [target_libdir, sysroot] = target_info.split("\n");
  log("Target libdir:", target_libdir);
  log("Sysroot: ", sysroot);

  const library_path = LIBRARY_PATHS[process.platform] || "LD_LIBRARY_PATH";

  return {
    cwd,
    [library_path]: target_libdir,
    SYSROOT: sysroot,
    RUST_BACKTRACE: "1",
  };
};

export let exec_notify = async (
  cmd: string,
  title: string,
  opts?: any
): Promise<string> => {
  log("Running command: ", cmd);

  // See issue #4
  let shell: boolean | string = process.env.SHELL || true;
  let proc = cp.spawn(cmd, {
    shell,
    ...opts,
  });

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
  _args: string,
  _no_output?: boolean
) => Promise<FlowistryResult<T>>;

export async function setup(
  context: vscode.ExtensionContext
): Promise<CallFlowistry | null> {
  let folders = vscode.workspace.workspaceFolders;
  if (!folders || folders.length === 0) {
    return null;
  }

  let workspace_root = folders[0].uri.fsPath;
  log("Workspace root", workspace_root);

  let cargo = `cargo +${TOOLCHAIN.channel}`;

  let version;
  try {
    let output = await exec_notify(
      `${cargo} flowistry -V`,
      "Waiting for Flowistry..."
    );
    version = output.split(" ")[1];
  } catch (e) {
    version = "";
  }

  if (version !== VERSION) {
    let components = TOOLCHAIN.components.map((c) => `-c ${c}`).join(" ");
    let rustup_cmd = `rustup toolchain install ${TOOLCHAIN.channel} ${components}`;
    try {
      await exec_notify(rustup_cmd, "Installing nightly Rust...");
    } catch (e: any) {
      let choice = await vscode.window.showErrorMessage(
        'Flowistry failed to install because rustup failed. Click "Show fix" to resolve, or click "Dismiss to attempt installation later.',
        "Show fix",
        "Dismiss"
      );

      if (choice === "Show fix") {
        open(
          "https://github.com/willcrichton/flowistry/blob/master/README.md#1-rustup-fails-on-installation"
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

      let cargo_cmd = `${cargo} install flowistry_ide --version ${VERSION} --force`;
      await exec_notify(
        cargo_cmd,
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
  return async <T>(args: string, no_output: boolean = false) => {
    let cmd = `${flowistry_cmd} ${args}`;

    let output;
    try {
      let editor = vscode.window.activeTextEditor;
      if (editor) {
        await editor.document.save();
      }

      output = await exec_notify(
        cmd,
        "Waiting for Flowistry...",
        flowistry_opts
      );
    } catch (e: any) {
      context.workspaceState.update("err_log", e);

      return {
        type: "build-error",
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
    if (output_typed.variant === "Err") {
      return {
        type: "analysis-error",
        error: output_typed.fields[0],
      };
    }

    return {
      type: "output",
      value: output_typed.fields[0],
    };
  };
}

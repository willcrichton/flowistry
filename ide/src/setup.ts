import * as vscode from "vscode";
import * as cp from "child_process";
import _ from "lodash";
import { Readable } from "stream";

import { log, CallFlowistry } from "./vsc_utils";
import { download } from "./download";

declare const VERSION: string;
declare const TOOLCHAIN: {
  channel: string;
  components: string[];
};

const SHOW_LOADER_THRESHOLD = 2000;

let exec_notify = async (
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

  let promise = new Promise<string>((resolve, reject) => {
    proc.addListener("close", (_) => {
      if (proc.exitCode !== 0) {
        reject(stderr().split("\n").slice(-1)[0]);
      } else {
        resolve(stdout());
      }
    });
    proc.addListener("error", (e) => {
      reject(e.toString());
    });
  });

  let outcome = await Promise.race([
    promise,
    new Promise<undefined>((resolve, _) =>
      setTimeout(resolve, SHOW_LOADER_THRESHOLD)
    ),
  ]);

  if (outcome === undefined) {
    outcome = await vscode.window.withProgress(
      {
        location: vscode.ProgressLocation.Notification,
        title,
        cancellable: true,
      },
      (_, token) => {
        token.onCancellationRequested((_) => {
          proc.kill("SIGINT");
        });
        return promise;
      }
    );
  }

  return outcome;
};



export async function setup(): Promise<CallFlowistry | null> {
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

  if (version != VERSION) {
    let components = TOOLCHAIN.components.join(",");
    let rustup_cmd = `rustup toolchain install ${TOOLCHAIN.channel} -c ${components}`;
    await exec_notify(rustup_cmd, "Installing nightly Rust...");

    try { 
      // FIXME: disabling downloads temporarily while fixing tar issue
      // await download();      
    } catch (e: any) {
      log("Install script failed with error:", e.toString());

      let cargo_cmd = `${cargo} install flowistry --version ${VERSION} --force`;
      await exec_notify(
        cargo_cmd,
        "Flowistry binaries not available, instead installing Flowistry crate from source... (this may take a minute)"
      );
    }

    if (version == "") {
      vscode.window.showInformationMessage(
        "Flowistry has successfully installed! Try selecting a variable in a function, then do: right click -> Flowistry -> Backward Highlight."
      );
    }
  }

  let rustc_path = await exec_notify(
    `rustup which --toolchain ${TOOLCHAIN.channel} rustc`,
    "Waiting for rustc..."
  );
  let target_info = await exec_notify(
    `${rustc_path} --print target-libdir --print sysroot`,
    "Waiting for rustc..."
  );
  let [target_libdir, sysroot] = target_info.split("\n");
  log("Target libdir:", target_libdir);
  log("Sysroot: ", sysroot);

  let call_flowistry: CallFlowistry = async (args) => {
    let cmd = `${cargo} flowistry ${args}`;
    let library_path;
    if (process.platform == "darwin") {
      library_path = "DYLD_LIBRARY_PATH";
    } else if (process.platform == "win32") {
      library_path = "LIB";
    } else {
      library_path = "LD_LIBRARY_PATH";
    }

    try {
      return exec_notify(cmd, "Waiting for Flowistry...", {
        cwd: workspace_root,
        [library_path]: target_libdir,
        SYSROOT: sysroot,
      });
    } catch (e: any) {
      throw `Flowistry failed to execute: ${e.toString()}`;
    }
  };

  return call_flowistry;
}

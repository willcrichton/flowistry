import * as vscode from "vscode";
import * as cp from "child_process";
import * as util from "util";
import _ from "lodash";
import { log, show_error, CallFlowistry } from "./vsc_utils";
import { Readable } from "stream";

declare const VERSION: string;
declare const CHANNEL: string;

let exec = (cmd: string, opts?: any): [Promise<string>, cp.ChildProcessWithoutNullStreams] => {
  log("Running command: ", cmd);
  let proc = cp.spawn(cmd, {
    shell: true,
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

  return [new Promise<string>((resolve, reject) => {
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
  }), proc];
};

const SHOW_LOADER_THRESHOLD = 1000;

export async function setup(): Promise<CallFlowistry | null> {
  let folders = vscode.workspace.workspaceFolders;
  if (!folders || folders.length === 0) {
    return null;
  }

  let workspace_root = folders[0].uri.fsPath;
  log("Workspace root", workspace_root);

  let cargo = `cargo +${CHANNEL}`;

  let fresh_install = false;
  try {
    await exec(`${cargo} flowistry -V`)[0];
  } catch (e) {
    let outcome = await vscode.window.showInformationMessage(
      "The Flowistry crate needs to be installed. Would you like to automatically install it now?",
      ...["Install", "Cancel"]
    );
    if (outcome === "Cancel") {
      return null;
    }

    await vscode.window.withProgress(
      {
        location: vscode.ProgressLocation.Notification,
        title: "Installing Flowistry crate... (this may take a few minutes)",
      },
      (_) =>
        exec(
          `rustup toolchain install ${CHANNEL} -c rust-src,rustc-dev,llvm-tools-preview && ${cargo} install flowistry --version ${VERSION}`
        )[0]
    );

    fresh_install = true;
  }

  if (fresh_install) {
    vscode.window.showInformationMessage(
      "Flowistry has successfully installed! Try selecting a variable in a function, then do: right click -> Flowistry -> Backward Highlight."
    );
  }

  let call_flowistry: CallFlowistry = async (args) => {
    let cmd = `${cargo} flowistry ${args}`;
    let [promise, proc] = exec(cmd, { cwd: workspace_root });

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
          title: "Waiting for Flowistry...",
          cancellable: true,
        },
        (_, token) => {
          token.onCancellationRequested((_) => proc.kill("SIGINT"));
          return promise;
        }
      );
    }

    return outcome;
  };

  return call_flowistry;
}

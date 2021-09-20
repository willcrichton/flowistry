import * as vscode from "vscode";
import * as cp from "child_process";
import * as util from "util";
import _ from "lodash";
import { log, show_error, CallFlowistry } from "./vsc_utils";
import { Readable } from "stream";

let exec = util.promisify(cp.exec);

const SHOW_LOADER_THRESHOLD = 1000;

export async function setup(): Promise<CallFlowistry | null> {
  let folders = vscode.workspace.workspaceFolders;
  if (!folders || folders.length === 0) {
    return null;
  }

  let workspace_root = folders[0].uri.fsPath;
  log("Workspace root", workspace_root);

  let fresh_install = false;
  try {
    await exec("cargo flowistry -V");
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
      (_) => exec("rustup toolchain install nightly -c rust-src,rustc-dev,llvm-tools-preview && cargo +nightly install flowistry")
    );

    fresh_install = true;
  }

  let get_libdir = async (): Promise<string> => {
    let { stdout } = await exec("rustc --print target-libdir", {
      cwd: workspace_root,
    });
    return stdout.trim();
  };

  let get_rustc_version = async (): Promise<{ [key: string]: string }> => {
    let { stdout } = await exec("rustc -vV", { cwd: workspace_root });
    return _.fromPairs(
      stdout
        .trim()
        .split("\n")
        .slice(1) // skip first line
        .map((line) => {
          let [_, k, v] = line.match(/([^:]*): (.*)/)!;
          return [k, v];
        })
    );
  };

  let get_flowistry_rustc_hash = async (): Promise<string> => {
    let { stdout } = await exec("cargo flowistry rustc_version", {
      cwd: workspace_root,
    });
    return stdout.trim();
  };

  let [lib_dir, rustc_version, flowistry_rustc_hash] = await Promise.all([
    get_libdir(),
    get_rustc_version(),
    get_flowistry_rustc_hash(),
  ]);

  let release = rustc_version.release;
  log("Rust version: ", release);
  if (!(release.includes("nightly") || release.includes("-dev"))) {
    show_error(
      "Flowistry only works on projects built with the nightly compiler. To fix this, consider running: rustup override set nightly"
    );
    return null;
  }

  log("Current rustc hash: ", rustc_version["commit-hash"]);
  log("Flowistry rustc hash: ", flowistry_rustc_hash);
  if (rustc_version["commit-hash"] !== flowistry_rustc_hash) {
    show_error(
      "Flowistry was compiled with a different nightly than the current one. Please recompile by running: cargo +nightly install flowistry"
    );
    return null;
  }

  log("Rustc target-libdir", lib_dir);

  if (fresh_install) {
    vscode.window.showInformationMessage(
      "Flowistry has successfully installed!"
    );
  }

  let call_flowistry: CallFlowistry = async (args) => {
    let env = {
      ...process.env,
      DYLD_LIBRARY_PATH: lib_dir,
      LD_LIBRARY_PATH: lib_dir,
    };
    let cmd = `cargo flowistry ${args}`;

    log("Running command:");
    log(cmd);

    let parts = cmd.split(" ");
    let proc = cp.spawn(parts[0], parts.slice(1), {
      env,
      cwd: workspace_root,
    });

    let read_stream = (stream: Readable): (() => string) => {
      let buffer: string[] = [];
      stream.setEncoding("utf8");
      stream.on("data", (data) => {
        buffer.push(data.toString());
      });
      return () => buffer.join("");
    };

    let stdout = read_stream(proc.stdout);
    let stderr = read_stream(proc.stderr);

    let promise = new Promise<string>((resolve, reject) => {
      proc.addListener("close", (_) => {
        resolve(stdout());
      });
      proc.addListener("error", (e) => {
        log(stderr());
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
          title: "Waiting for Flowistry...",
          cancellable: true,
        },
        (_, token) => {
          token.onCancellationRequested((_) => proc.kill("SIGINT"));
          return promise;
        }
      );
    }

    return outcome.trim();
  };

  return call_flowistry;
}

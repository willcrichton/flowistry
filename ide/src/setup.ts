import * as vscode from "vscode";
import * as cp from "child_process";
import * as util from "util";
import { log, show_error, CallFlowistry } from "./vsc_utils";

export const exec = util.promisify(cp.exec);

export async function setup(): Promise<CallFlowistry | null> {
  let folders = vscode.workspace.workspaceFolders;
  if (!folders || folders.length === 0) {
    return null;
  }
  let workspace_root = folders[0].uri.fsPath;
  log("Workspace root", workspace_root);

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

    await exec("cargo +nightly install flowistry");
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

  log("Rust version: ", rustc_version["release"]);
  if (!rustc_version["release"].includes("nightly")) {
    show_error(
      "Flowistry can only work on projects building with the nightly compiler. To fix this, consider running: rustup override set nightly"
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

  let call_flowistry: CallFlowistry = async (args) => {
    let env = {
      ...process.env,
      DYLD_LIBRARY_PATH: lib_dir,
      LD_LIBRARY_PATH: lib_dir,
    };
    let cmd = `cargo flowistry ${args}`;

    log("Running command:");
    log(cmd);

    let { stdout } = await exec(cmd, { env, cwd: workspace_root });
    return stdout.trim();
  };

  return call_flowistry;
}

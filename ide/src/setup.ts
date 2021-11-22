import * as vscode from "vscode";
import * as cp from "child_process";
import _ from "lodash";
import { Readable } from "stream";
import open from "open";

import { Result } from "./types";
import { log, CallFlowistry } from "./vsc_utils";
import { download } from "./download";

declare const VERSION: string;
declare const TOOLCHAIN: {
  channel: string;
  components: string[];
};

const SHOW_LOADER_THRESHOLD = 2000;

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

  let promise = new Promise<string>((resolve, reject) => {
    proc.addListener("close", (_) => {
      if (proc.exitCode !== 0) {
        reject(stderr());
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

  if (version != VERSION) {
    let components = TOOLCHAIN.components.map(c => `-c ${c}`).join(" ");
    let rustup_cmd = `rustup toolchain install ${TOOLCHAIN.channel} ${components}`;
    try {
      await exec_notify(rustup_cmd, "Installing nightly Rust...");
    } catch (e: any) {
      let choice = await vscode.window.showErrorMessage(
        "Flowistry failed to install because rustup failed. Click \"Show fix\" to resolve, or click \"Dismiss\ to attempt installation later.",
        "Show fix",
        "Dismiss"
      );

      if (choice == "Show fix") {
        open("https://github.com/willcrichton/flowistry/blob/master/README.md#rustup-fails-on-installation");
        await vscode.window.showInformationMessage("Click \"Continue\" once you have completed the fix.", "Continue");
      } else {
        return null;
      }
    }


    try {
      await download();
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

  const tdcp = new (class implements vscode.TextDocumentContentProvider {
    readonly uri = vscode.Uri.parse("flowistry://build-error");
    readonly eventEmitter = new vscode.EventEmitter<vscode.Uri>();
    contents: string = "";

    provideTextDocumentContent(
      _uri: vscode.Uri
    ): vscode.ProviderResult<string> {
      return `Flowistry could not run because your project failed to build with error:\n${this.contents}`;
    }

    get onDidChange(): vscode.Event<vscode.Uri> {
      return this.eventEmitter.event;
    }
  })();

  context.subscriptions.push(
    vscode.workspace.registerTextDocumentContentProvider("flowistry", tdcp)
  );

  return async <T>(args: string) => {
    let cmd = `${flowistry_cmd} ${args}`;
    let flowisty_opts = await get_flowistry_opts(workspace_root);

    let output;
    try {
      let editor = vscode.window.activeTextEditor;
      if (editor) {
        await editor.document.save();
      }

      output = await exec_notify(cmd, "Waiting for Flowistry...", flowisty_opts);
    } catch (e: any) {
      tdcp.contents = e.toString();
      tdcp.eventEmitter.fire(tdcp.uri);
      let doc = await vscode.workspace.openTextDocument(tdcp.uri);
      await vscode.window.showTextDocument(doc, vscode.ViewColumn.Beside);
      return null;
    }

    let output_typed: Result<T> = JSON.parse(output);
    if (output_typed.variant === "Err") {
      throw output_typed.fields[0];
    }

    return output_typed.fields[0];
  };
}

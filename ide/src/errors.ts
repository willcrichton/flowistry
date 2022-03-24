import * as cp from "child_process";
import _ from "lodash";
import newGithubIssueUrl from "new-github-issue-url";
import open from "open";
import os from "os";
import vscode from "vscode";

import { globals } from "./extension";
import { log, logs } from "./logging";

interface BuildError {
  type: "build-error";
  error: string;
}
interface AnalysisError {
  type: "analysis-error";
  error: string;
}
export type FlowistryError = BuildError | AnalysisError;
interface FlowistryOutput<T> {
  type: "output";
  value: T;
}
export type FlowistryResult<T> = FlowistryOutput<T> | FlowistryError;

export let ok = <T>(value: T): FlowistryResult<T> => ({
  type: "output",
  value,
});

export const is_ok = <T>(res: FlowistryResult<T>): res is FlowistryOutput<T> =>
  res.type === "output";

class ErrorProvider implements vscode.TextDocumentContentProvider {
  readonly uri = vscode.Uri.parse("flowistry://build-error");
  readonly eventEmitter = new vscode.EventEmitter<vscode.Uri>();
  contents: string = "";

  constructor(context: vscode.ExtensionContext) {
    context.subscriptions.push(
      vscode.workspace.registerTextDocumentContentProvider("flowistry", this)
    );
  }

  provideTextDocumentContent(_uri: vscode.Uri): vscode.ProviderResult<string> {
    return `Flowistry could not run because your project failed to build with error:\n${this.contents}`;
  }

  get onDidChange(): vscode.Event<vscode.Uri> {
    return this.eventEmitter.event;
  }
}

export class ErrorPane {
  tdcp: ErrorProvider;
  doc: vscode.TextDocument;
  editor?: vscode.TextEditor;

  constructor(tdcp: ErrorProvider, doc: vscode.TextDocument) {
    this.tdcp = tdcp;
    this.doc = doc;
  }

  static load = async (
    context: vscode.ExtensionContext
  ): Promise<ErrorPane> => {
    let tdcp = new ErrorProvider(context);
    let doc = await vscode.workspace.openTextDocument(tdcp.uri);
    return new ErrorPane(tdcp, doc);
  };

  show = async (error: string) => {
    this.tdcp.contents = error;
    this.tdcp.eventEmitter.fire(this.tdcp.uri);
    if (!this.editor) {
      this.editor = await vscode.window.showTextDocument(this.doc, {
        viewColumn: vscode.ViewColumn.Beside,
        preserveFocus: true,
      });
    }
  };

  hide = async () => {
    if (this.editor) {
      // TODO: TextEditor.hide is deprecated, but I could not figure out an
      // alternative way to hide the error panel. The docs say to use
      // workbench.action.closeActiveEditor, but that requires the error panel
      // to be the active editor. I couldn't find a method to make the error panel
      // active before calling closeActiveEditor, since showTextDocument always creates
      // a new editor and doesn't bring up the existing one.
      this.editor.hide();
      this.editor = undefined;
    }
  };
}

export let show_error_dialog = async (err: string) => {
  let outcome = await vscode.window.showErrorMessage(
    `Flowistry error: ${err}`,
    "Report bug",
    "Dismiss"
  );
  if (outcome === "Report bug") {
    let log_url = null;
    try {
      log_url = cp.execSync("curl --data-binary @- https://paste.rs/", {
        input: logs.join("\n"),
      });
    } catch (e) {
      log("Failed to call to paste.rs: ", e.toString());
    }

    let bts = "```";
    let log_text = log_url !== null ? `\n**Full log:** ${log_url}` : ``;
    let url = newGithubIssueUrl({
      user: "willcrichton",
      repo: "flowistry",
      body: `# Problem
<!-- Please describe the problem and how you encountered it. -->

# Logs
<!-- You don't need to add or change anything below this point. -->

**OS:** ${os.platform()} (${os.release()})
**VSCode:** ${vscode.version}
**Error message**
${bts}
${err}
${bts}
${log_text}`,
    });

    await open(url);
  }
};

export const show_error = async (error: BuildError | AnalysisError) => {
  if (error.type === "build-error") {
    await globals.error_pane.show(error.error);
  } else {
    await show_error_dialog(error.error);
  }
};

export let hide_error = () => globals.error_pane.hide();

export async function last_error(context: vscode.ExtensionContext) {
  let error = context.workspaceState.get("err_log") as string;
  let flowistry_err: BuildError = { type: "build-error", error };
  await show_error(flowistry_err);
}

// The module 'vscode' contains the VS Code extensibility API
// Import the module and reference it with the alias vscode in your code below
import * as vscode from "vscode";
import * as cp from "child_process";
import * as util from "util";

const exec = util.promisify(cp.exec);

let channel = vscode.window.createOutputChannel("Flowistry");
let log = (...strs: any[]) => {
  channel.appendLine(strs.map((obj) => String(obj)).join("\t"));
};

interface SliceRange {
  start_line: number;
  start_col: number;
  end_line: number;
  end_col: number;
}

interface SliceOutput {
  ranges: SliceRange[];
}

// this method is called when your extension is activated
// your extension is activated the very first time the command is executed
export async function activate(context: vscode.ExtensionContext) {
  log("flowistry is activated");

  let { stdout } = await exec("rustc --print target-libdir");
  let lib_dir = stdout.trim();
  log("Rustc target-libdir", lib_dir);

  let decoration_type = vscode.window.createTextEditorDecorationType({
    backgroundColor: new vscode.ThemeColor(
      "editor.findMatchHighlightBackground"
    ),
  });

  let folders = vscode.workspace.workspaceFolders;
  if (!folders || folders.length === 0) {
    return;
  }
  let workspace_root = folders[0].uri.fsPath;
  log("Workspace root", workspace_root);

  let slice = async (
    direction: "backward" | "forward",
    type: "highlight" | "select"
  ) => {
    let active_editor = vscode.window.activeTextEditor;
    if (!active_editor) {
      return;
    }

    let doc = active_editor.document;
    let selection = active_editor.selection;

    let env = {
      ...process.env,
      DYLD_LIBRARY_PATH: lib_dir,
      LD_LIBRARY_PATH: lib_dir,
    };
    let subcmd = `${direction}_slice`;
    let cmd = `cargo flowistry ${subcmd} ${doc.fileName} ${doc.offsetAt(
      selection.start
    )} ${doc.offsetAt(selection.end)}`;

    try {
      log("Running command:");
      log(cmd);

      let { stdout } = await exec(cmd, { env, cwd: workspace_root });

      let lines = stdout.trim().split("\n");
      log(lines);
      let last_line = lines[lines.length - 1];
      let slice_output: SliceOutput = JSON.parse(last_line);

      let ranges = slice_output.ranges.map(
        (slice_range: any) =>
          new vscode.Range(
            doc.positionAt(slice_range.start),
            doc.positionAt(slice_range.end)
          )
      );

      if (ranges.length === 0) {
        let selected_text = active_editor.document.getText(selection);
        vscode.window.showInformationMessage(
          `Slice on "${selected_text}" did not generate any results`
        );
        return;
      }

      if (type === "select") {
        active_editor.selections = ranges.map(
          (range) => new vscode.Selection(range.start, range.end)
        );
      } else {
        active_editor.setDecorations(decoration_type, ranges);

        let callback = vscode.workspace.onDidChangeTextDocument((event) => {
          if (!active_editor) {
            return;
          }
          if (event.document !== active_editor.document) {
            return;
          }
          active_editor.setDecorations(decoration_type, []);
          callback.dispose();
        });
      }
    } catch (exc) {
      log("ERROR", exc);
      vscode.window.showErrorMessage(`Flowistry failed with error: ${exc}`);
    }
  };

  // The command has been defined in the package.json file
  // Now provide the implementation of the command with registerCommand
  // The commandId parameter must match the command field in package.json

  let register_with_opts = (name: string, f: () => void) => {
    let disposable = vscode.commands.registerCommand(`flowistry.${name}`, f);
    context.subscriptions.push(disposable);
  };

  ["backward", "forward"].forEach((direction: any) => {
    ["highlight", "select"].forEach((type: any) => {
      register_with_opts(`${direction}_${type}`, () => slice(direction, type));
    });
  });
}

export function deactivate() {}

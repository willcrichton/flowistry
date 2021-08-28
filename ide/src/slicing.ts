import * as vscode from "vscode";
import { SliceOutput } from "./types";
import { log, show_error, decoration_type, CallFlowistry } from "./vsc_utils";

export async function slice(
  call_flowistry: CallFlowistry,
  direction: "backward" | "forward",
  type: "highlight" | "select"
) {
  let active_editor = vscode.window.activeTextEditor;
  if (!active_editor) {
    return;
  }

  let doc = active_editor.document;
  let selection = active_editor.selection;

  try {
    let subcmd = `${direction}_slice`;
    let cmd = `${subcmd} ${doc.fileName} ${doc.offsetAt(
      selection.start
    )} ${doc.offsetAt(selection.end)}`;
    let stdout = await call_flowistry(cmd);

    let lines = stdout.split("\n");
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
    show_error(exc);
  }
}

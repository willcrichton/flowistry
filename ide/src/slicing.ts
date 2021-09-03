import * as vscode from "vscode";
import { SliceOutput, Range } from "./types";
import {
  log,
  show_error,
  CallFlowistry,
  to_vsc_range,
} from "./vsc_utils";

export let highlight_type = vscode.window.createTextEditorDecorationType({
  backgroundColor: 'rgb(250, 223, 203)'
});

export let select_type = vscode.window.createTextEditorDecorationType({
  backgroundColor: 'rgb(186, 220, 199)'
});

export function highlight_ranges(ranges: Range[], editor: vscode.TextEditor, type = highlight_type) {
  editor.setDecorations(
    type,
    ranges.map((range) => to_vsc_range(range, editor.document))
  );

  let callback = vscode.workspace.onDidChangeTextDocument((event) => {
    if (!editor) {
      return;
    }
    if (event.document !== editor.document) {
      return;
    }
    editor.setDecorations(type, []);
    callback.dispose();
  });
}

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

    if (slice_output.ranges.length === 0) {
      let selected_text = active_editor.document.getText(selection);
      vscode.window.showInformationMessage(
        `Slice on "${selected_text}" did not generate any results`
      );
      return;
    }

    if (type === "select") {
      active_editor.selections = slice_output.ranges.map((range) => {
        let vsc_range = to_vsc_range(range, doc);
        return new vscode.Selection(vsc_range.start, vsc_range.end);
      });
    } else {
      highlight_ranges(slice_output.ranges, active_editor);
    }
  } catch (exc) {
    log("ERROR", exc);
    show_error(exc);
  }
}

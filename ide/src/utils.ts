import * as vscode from "vscode";
import { CommandOutput, Range } from "./types";
import { log, show_error, CallFlowistry, to_vsc_range } from "./vsc_utils";
import _ from "lodash";

export let highlight_type = vscode.window.createTextEditorDecorationType({
  backgroundColor: new vscode.ThemeColor("editor.symbolHighlightBackground"),
});

export let hide_type = vscode.window.createTextEditorDecorationType({
  opacity: "0.4",
});

let style = {
  color: "white",
  backgroundColor: "rgb(153, 222, 179)",
  fontWeight: "bold",
};
export let select_type = vscode.window.createTextEditorDecorationType({
  before: {
    contentText: "❰",
    margin: "0 5px 0 0",
    ...style,
  },
  after: {
    contentText: "❱",
    margin: "0 0 0 5px",
    ...style,
  },
});

export let invert_ranges = (container: Range, pieces: Range[]): Range[] => {
  let filename = container.filename;
  let pieces_sorted = _.sortBy(pieces, (r) => r.start);

  let new_ranges: Range[] = [];
  let start = container.start;
  pieces_sorted.forEach((r) => {
    if (r.start < start) {
      start = Math.max(r.end, start);
      return;
    }

    let end = r.start;
    new_ranges.push({
      start,
      end,
      filename,
    });

    start = Math.max(start, r.end);
  });

  new_ranges.push({
    start,
    end: container.end,
    filename,
  });

  return new_ranges;
};

export let highlight_slice = (
  editor: vscode.TextEditor,
  container: Range,
  seeds: Range[],
  slice: Range[]
) => {
  highlight_ranges(seeds, editor, select_type);
  highlight_ranges(invert_ranges(container, slice), editor, hide_type);
};

export function highlight_ranges(
  ranges: Range[],
  editor: vscode.TextEditor,
  type: vscode.TextEditorDecorationType = highlight_type
) {
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

export async function display_subcmd_results(
  call_flowistry: CallFlowistry,
  action: string,
  subcmd: string,
  display_type: "highlight" | "select",
  flags: string = ""
) {
  let active_editor = vscode.window.activeTextEditor;
  if (!active_editor) {
    return;
  }

  let doc = active_editor.document;
  let selection = active_editor.selection;

  try {
    let start = doc.offsetAt(selection.start);
    let end = doc.offsetAt(selection.end);
    let cmd = `${subcmd} ${doc.fileName} ${start} ${end} ${flags}`;
    let command_output_maybe = await call_flowistry<CommandOutput>(cmd);
    if (command_output_maybe === null) {
      return;
    }
    let command_output: CommandOutput = command_output_maybe;

    if (command_output.ranges.length === 0) {
      let selected_text = active_editor.document.getText(selection);
      vscode.window.showInformationMessage(
        `${action} on "${selected_text}" did not generate any results`
      );
      return;
    }

    if (display_type === "select") {
      active_editor.selections = command_output.ranges.map((range) => {
        let vsc_range = to_vsc_range(range, doc);
        return new vscode.Selection(vsc_range.start, vsc_range.end);
      });
    } else {
      highlight_slice(
        active_editor,
        command_output.body_span,
        command_output.selected_spans,
        command_output.ranges
      );
    }
  } catch (exc) {
    log("ERROR", exc);
    show_error(exc);
  }
}

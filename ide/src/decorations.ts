import _ from "lodash";
import * as vscode from "vscode";

import { Range, to_vsc_range } from "./range";

export let highlight_type = vscode.window.createTextEditorDecorationType({
  backgroundColor: new vscode.ThemeColor("editor.symbolHighlightBackground"),
});

export let emphasis_type = vscode.window.createTextEditorDecorationType({
  textDecoration: "wavy underline lime",
});

export let hide_type = vscode.window.createTextEditorDecorationType({
  opacity: "0.4",
});

export let select_type = vscode.window.createTextEditorDecorationType({
  backgroundColor: new vscode.ThemeColor("editor.wordHighlightBackground"),
});

export let invert_ranges = (container: Range, pieces: Range[]): Range[] => {
  let filename = container.filename;
  let pieces_sorted = _.sortBy(pieces, (r) => r.char_start).filter(
    (r) =>
      container.char_start <= r.char_start && r.char_end <= container.char_end
  );

  let new_ranges: Range[] = [];
  let start = container.char_start;
  pieces_sorted.forEach((r) => {
    if (r.char_start < start) {
      start = Math.max(r.char_end, start);
      return;
    }

    let end = r.char_start;
    new_ranges.push({
      char_start: start,
      char_end: end,
      filename,
    });

    start = Math.max(start, r.char_end);
  });

  new_ranges.push({
    char_start: start,
    char_end: container.char_end,
    filename,
  });

  return new_ranges;
};

export let highlight_slice = (
  editor: vscode.TextEditor,
  containers: Range[],
  seeds: Range[],
  slice: Range[]
) => {
  highlight_ranges(seeds, editor, select_type);
  let hide_ranges = containers
    .map((container) => invert_ranges(container, slice))
    .flat();
  highlight_ranges(hide_ranges, editor, hide_type);
};

export let emphasize_ranges = (editor: vscode.TextEditor, ranges: Range[]) => {
  highlight_ranges(ranges, editor, emphasis_type);
};

export function highlight_ranges(
  ranges: Range[],
  editor: vscode.TextEditor,
  type: vscode.TextEditorDecorationType
) {
  editor.setDecorations(
    type,
    ranges.map((range) => to_vsc_range(range, editor.document))
  );
}

export let clear_ranges = (editor: vscode.TextEditor) => {
  [highlight_type, hide_type, select_type, emphasis_type].forEach((type) => {
    editor.setDecorations(type, []);
  });
};

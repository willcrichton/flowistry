import _ from "lodash";
import * as vscode from "vscode";

import { Range, to_vsc_range } from "./range";

export let highlight_type = vscode.window.createTextEditorDecorationType({
  backgroundColor: new vscode.ThemeColor("editor.symbolHighlightBackground"),
});

export let emphasis_type = vscode.window.createTextEditorDecorationType({
  dark: {
    backgroundColor: "rgba(255, 255, 255, 0.07)",
  },
  light: {
    backgroundColor: "rgba(0, 0, 0, 0.05)",
  },
});

export let slice_type = vscode.window.createTextEditorDecorationType({
  opacity: "1.0",
});

export let hide_type = vscode.window.createTextEditorDecorationType({
  opacity: "0.3",
});

export let select_type = vscode.window.createTextEditorDecorationType({
  dark: {
    backgroundColor: "rgba(255, 255, 255, 0.1)",
  },
  light: {
    backgroundColor: "rgba(0, 0, 0, 0.1)",
  },
});

export let invert_ranges = (container: Range, pieces: Range[]): Range[] => {
  let filename = container.filename;
  let pieces_sorted = _.sortBy(pieces, (r) => r.start).filter(
    (r) =>
      container.start <= r.start && r.end <= container.end
  );

  let new_ranges: Range[] = [];
  let start = container.start;
  pieces_sorted.forEach((r) => {
    if (r.start < start) {
      start = Math.max(r.end, start);
      return;
    }

    let end = r.start;
    new_ranges.push({
      start: start,
      end: end,
      filename,
    });

    start = Math.max(start, r.end);
  });

  new_ranges.push({
    start: start,
    end: container.end,
    filename,
  });

  return new_ranges;
};

export let highlight_slice = (
  editor: vscode.TextEditor,
  containers: Range[],
  seeds: Range[],
  slice: Range[],
  direct_influence: Range[]
) => {
  highlight_ranges(seeds, editor, select_type);
  let hide_ranges = containers
    .map((container) => invert_ranges(container, slice))
    .flat();
  highlight_ranges(hide_ranges, editor, hide_type);
  highlight_ranges(slice, editor, slice_type);
  highlight_ranges(direct_influence, editor, emphasis_type);
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
  [highlight_type, hide_type, select_type, slice_type, emphasis_type].forEach(
    (type) => {
      editor.setDecorations(type, []);
    }
  );
};

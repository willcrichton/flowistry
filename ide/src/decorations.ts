import _ from "lodash";
import * as vscode from "vscode";

import {
  Interval,
  Range,
  interval_to_range,
  range_to_interval,
  to_vsc_range,
} from "./range";

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

export let invert_ranges = (
  container: Range,
  pieces: Range[],
  doc: vscode.TextDocument
): Range[] => {
  let icontainer = range_to_interval(container, doc);
  let ipieces = pieces.map((piece) => range_to_interval(piece, doc));

  let filename = container.filename;
  let pieces_sorted = _.sortBy(ipieces, (r) => r[0]).filter(
    (r) => icontainer[0] <= r[0] && r[1] <= icontainer[1]
  );

  let new_ranges: Interval[] = [];
  let start = icontainer[0];
  pieces_sorted.forEach((r) => {
    if (r[0] < start) {
      start = Math.max(r[1], start);
      return;
    }

    let end = r[0];
    new_ranges.push([start, end]);

    start = Math.max(start, r[1]);
  });

  new_ranges.push([start, icontainer[1]]);

  return new_ranges.map((intvl) => interval_to_range(intvl, filename, doc));
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
    .map((container) => invert_ranges(container, slice, editor.document))
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
    ranges.map((range) => to_vsc_range(range))
  );
}

export let clear_ranges = (editor: vscode.TextEditor) => {
  [highlight_type, hide_type, select_type, slice_type, emphasis_type].forEach(
    (type) => {
      editor.setDecorations(type, []);
    }
  );
};

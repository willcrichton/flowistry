import * as vscode from "vscode";
import { Range } from "./types";
import { to_vsc_range } from "./vsc_utils";
import _ from "lodash";

export let highlight_type = vscode.window.createTextEditorDecorationType({
  backgroundColor: new vscode.ThemeColor("editor.symbolHighlightBackground"),
});

export let hide_type = vscode.window.createTextEditorDecorationType({
  opacity: "0.4",
});

export let select_type = vscode.window.createTextEditorDecorationType({
  backgroundColor: new vscode.ThemeColor("editor.wordHighlightBackground"),
});

export let invert_ranges = (container: Range, pieces: Range[]): Range[] => {
  let filename = container.filename;
  let pieces_sorted = _.sortBy(pieces, (r) => r.start).filter(
    (r) => container.start <= r.start && r.end <= container.end
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
  [highlight_type, hide_type, select_type].forEach((type) => {
    editor.setDecorations(type, []);
  });
};

// Ensure a value can be changed by-reference
export class Cell<T> {
  t: T;
  constructor(t: T) {
    this.t = t;
  }
  set(t: T) {
    this.t = t;
  }
  get(): T {
    return this.t;
  }
}

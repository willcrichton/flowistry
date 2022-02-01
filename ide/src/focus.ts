import * as vscode from "vscode";
import { highlight_slice, clear_ranges } from "./utils";
import { Range } from "./types";
import {  CallFlowistry } from "./vsc_utils";
import _ from "lodash";
import IntervalTree from "@flatten-js/interval-tree";

interface Slice {
  range: Range;
  slice: Range[];
}
interface Focus {
  slices: Slice[];
  body_range: Range;
  arg_range: Range;
}

type Interval = [number, number];

interface FocusState {
  mark: vscode.Selection | null;
  focus: Focus;
  ranges: IntervalTree<Slice>;
  disposable: vscode.Disposable;
}

let state: FocusState | null = null;

let initialize = async (call_flowistry: CallFlowistry) => {
  let active_editor = vscode.window.activeTextEditor;
  if (!active_editor) {
    return;
  }

  let doc = active_editor.document;
  let selection = active_editor.selection;

  let cmd = `focus ${doc.fileName} ${doc.offsetAt(selection.anchor)}`;
  let focus = await call_flowistry<Focus>(cmd);
  if (!focus) {
    return;
  }

  let ranges = new IntervalTree();
  focus.slices.forEach((slice) => {
    ranges.insert([slice.range.start, slice.range.end], slice.slice);
  });

  let disposable = vscode.window.onDidChangeTextEditorSelection(render);
  state = { disposable, focus, ranges, mark: null };
};

let render = () => {
  if (!state) {
    throw `Tried to render while state is invalid.`;
  }

  let active_editor = vscode.window.activeTextEditor!;
  let doc = active_editor.document;
  let { start, end } = state.mark || active_editor.selection;
  let query: Interval = [doc.offsetAt(start), doc.offsetAt(end)];

  let is_contained = (child: Interval, parent: Interval): boolean =>
    parent[0] <= child[0] && child[1] <= parent[1];

  let result = state.ranges.search(query, (v, k) => [[k.low, k.high], v]);
  let [contained, others] = _.partition(result, ([k]) =>
    is_contained(k, query)
  );

  console.log("query", query);
  console.log("result", result);

  let final;
  if (contained.length > 0) {
    final = contained;
    console.log("contained", contained);
  } else {
    let [containing, adjacent] = _.partition(others, ([k]) =>
      is_contained(query, k)
    );
    containing = _.sortBy(containing, ([k]) => k[1] - k[0]);
    final = adjacent.concat(containing.slice(0, 1));
    console.log("adjacent", adjacent);
    console.log("containing", containing);
  }

  let seeds = final.map(([k]) => ({
    start: k[0],
    end: k[1],
    filename: "",
  }));
  seeds = _.uniqWith(seeds, _.isEqual);
  let slice = final.map(([_k, v]) => v).flat();

  if (seeds.length > 0) {
    highlight_slice(active_editor, [state.focus.body_range, state.focus.arg_range], seeds, slice);
  } else {
    clear_ranges(active_editor);
  }
};

export let focus_mark = async (call_flowistry: CallFlowistry) => {
  let active_editor = vscode.window.activeTextEditor;
  if (!active_editor) {
    return;
  }

  if (!state) {
    await initialize(call_flowistry);
  }

  state!.mark = active_editor.selection;
  render();
};

export let focus_unmark = async (call_flowistry: CallFlowistry) => {
  let active_editor = vscode.window.activeTextEditor;
  if (!active_editor) {
    return;
  }

  if (!state) {
    await initialize(call_flowistry);
  }

  state!.mark = null;
  render();
};

export let focus = async (call_flowistry: CallFlowistry) => {
  if (state !== null) {
    clear_ranges(vscode.window.activeTextEditor!);
    state.disposable.dispose();
    state = null;
  } else {
    await initialize(call_flowistry);
    render();
  }
};

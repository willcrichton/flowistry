import * as vscode from "vscode";
import { highlight_slice } from "./utils";
import { Range } from "./types";
import { log, show_error, CallFlowistry } from "./vsc_utils";
import _ from "lodash";
import IntervalTree from "@flatten-js/interval-tree";

interface Slice {
  range: Range;
  slice: Range[];
}
interface Focus {
  slices: Slice[];
  body_span: Range;
}

export let focus = async (call_flowistry: CallFlowistry) => {
  let active_editor = vscode.window.activeTextEditor;
  if (!active_editor) {
    return;
  }

  let doc = active_editor.document;
  let selection = active_editor.selection;

  try {
    let cmd = `focus ${doc.fileName} ${doc.offsetAt(selection.anchor)}`;
    let focus = await call_flowistry<Focus>(cmd);
    if (!focus) {
      return;
    }

    let ranges = new IntervalTree();
    focus.slices.forEach((slice) => {
      ranges.insert([slice.range.start, slice.range.end], slice.slice);
    });

    type Interval = [number, number];
    let render = () => {
      let { start, end } = active_editor!.selection;
      let query: Interval = [doc.offsetAt(start), doc.offsetAt(end)];

      let is_contained = (child: Interval, parent: Interval): boolean =>
        parent[0] <= child[0] && child[1] <= parent[1];

      let result = ranges.search(query, (v, k) => [[k.low, k.high], v]);
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
        // others = _.sortBy(others, ([k]) => k[1] - k[0]);
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

      // let seeds = result.map(([k]) => k);
      // let slice = result.map(([_k, v]) => v).flat();
      // console.log(seeds, slice);
      highlight_slice(active_editor!, focus!.body_span, seeds, slice);
    };

    let d1 = vscode.window.onDidChangeTextEditorSelection(render);
    let d2 = vscode.workspace.onDidSaveTextDocument(() => {
      d1.dispose();
      d2.dispose();
    });

    render();
  } catch (exc: any) {
    log("ERROR", exc);
    show_error(exc);
  }
};

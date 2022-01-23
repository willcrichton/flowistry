import * as vscode from "vscode";
import { highlight_ranges } from "./slicing";
import { Range } from "./types";
import { log, show_error, CallFlowistry } from "./vsc_utils";
import _ from "lodash";

interface Decomposition {
  chunks: Range[][];
}

/*
[
    f'rgb({r:.02}, {g:.02}, {b:.02}, 0.2)'
    for (r, g, b) in sns.color_palette('pastel', n_colors=20)
]
*/
let palette = [
  "rgba(161, 201, 244, 0.2)",
  "rgba(255, 180, 130, 0.2)",
  "rgba(141, 229, 161, 0.2)",
  "rgba(255, 159, 155, 0.2)",
  "rgba(208, 187, 255, 0.2)",
  "rgba(222, 187, 155, 0.2)",
  "rgba(250, 176, 228, 0.2)",
  "rgba(207, 207, 207, 0.2)",
  "rgba(255, 254, 163, 0.2)",
  "rgba(185, 242, 240, 0.2)",
].map((backgroundColor) =>
  vscode.window.createTextEditorDecorationType({
    backgroundColor,
  })
);

export let decompose = async (call_flowistry: CallFlowistry) => {
  let active_editor = vscode.window.activeTextEditor;
  if (!active_editor) {
    return;
  }

  let doc = active_editor.document;
  let selection = active_editor.selection;

  try {
    let cmd = `decompose ${doc.fileName} ${doc.offsetAt(selection.anchor)}`;
    let decomp = await call_flowistry<Decomposition>(cmd);
    if (!decomp) {
      return;
    }

    decomp.chunks.forEach((chunk, i) => {
      highlight_ranges(chunk!, active_editor!, palette[i]);
    });
  } catch (exc: any) {
    log("ERROR", exc);
    show_error(exc);
  }
};

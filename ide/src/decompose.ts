import * as vscode from "vscode";
import { highlight_ranges } from "./slicing";
import { Range } from "./types";
import { log, show_error, CallFlowistry } from "./vsc_utils";
import _ from "lodash";

interface Decomposition {
  chunks: [number, Range[][]][];
}

/*
[
    f'rgb({r:.02}, {g:.02}, {b:.02}, 0.2)'
    for (r, g, b) in sns.color_palette('pastel', n_colors=20)
]
*/
let palette = [
  "rgba(31, 119, 180, 0.5)",
  "rgba(255, 127, 14, 0.5)",
  "rgba(44, 160, 44, 0.5)",
  "rgba(214, 39, 40, 0.5)",
  "rgba(148, 103, 189, 0.5)",
  "rgba(140, 86, 75, 0.5)",
  "rgba(227, 119, 194, 0.5)",
  "rgba(127, 127, 127, 0.5)",
  "rgba(188, 189, 34, 0.5)",
  "rgba(23, 190, 207, 0.5)",
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

    const panel = vscode.window.createWebviewPanel(
      "flowistry.decomp",
      `Flowistry: decomp`,
      vscode.ViewColumn.Beside,
      {
        enableScripts: true,
      }
    );
    panel.webview.html = `
<!DOCTYPE html>
<html>      
<body class="">
  <div id="app">
      <input type="range" id="range" min="0" max="${decomp.chunks.length}" />
  </div>
  <script>
      const vscode = window.acquireVsCodeApi();
      document.getElementById('range').addEventListener('input', function() {
        vscode.postMessage(this.value);
      })
  </script>
</body>        
</html>        
`;

    let show_chunks = (chunks: Range[][]) => {
      let editor = active_editor!;
      palette.forEach(type => {
        editor.setDecorations(type, []);
      });

      chunks.forEach((chunk, i) => {
        highlight_ranges(chunk, editor, palette[i]);
      });
    };
    show_chunks(decomp.chunks[Math.ceil(decomp.chunks.length / 2)][1])

    panel.webview.onDidReceiveMessage((i) => {
      show_chunks(decomp!.chunks[i][1]);
    });
    
  } catch (exc: any) {
    log("ERROR", exc);
    show_error(exc);
  }
};

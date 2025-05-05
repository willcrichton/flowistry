import _ from "lodash";
import * as vscode from "vscode";

import { highlight_ranges, highlight_simple_ranges } from "./decorations";
import { is_ok, show_error } from "./errors";
import { globals } from "./extension";
import { SimpleRange } from "./range";

interface Decomposition {
  chunks: [number, SimpleRange[][]][];
}

/*
[
    f'rgb({r:.02}, {g:.02}, {b:.02}, 0.2)'
    for (r, g, b) in sns.color_palette('pastel', n_colors=20)
]
*/
let colors = [
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
];
_.range(3).forEach(() => {
  colors = colors.concat(colors);
});
let palette = colors.map((backgroundColor) =>
  vscode.window.createTextEditorDecorationType({
    backgroundColor,
  })
);

export let decompose = async () => {
  let active_editor = vscode.window.activeTextEditor;
  if (!active_editor) {
    return;
  }

  let doc = active_editor.document;
  let selection = active_editor.selection;

  let cmd = [
    "decompose",
    doc.fileName,
    selection.anchor.line.toString(),
    selection.anchor.character.toString(),
  ];
  let decomp_res = await globals.call_flowistry<Decomposition>(cmd);
  if (!is_ok(decomp_res)) {
    return show_error(decomp_res);
  }
  let decomp = decomp_res.value;

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

  let show_chunks = (chunks: SimpleRange[][]) => {
    let editor = active_editor!;
    palette.forEach((type) => {
      editor.setDecorations(type, []);
    });

    chunks.forEach((chunk, i) => {
      highlight_simple_ranges(chunk, editor, palette[i]);
    });
  };

  // show_chunks(decomp.chunks);

  let init = Math.min(
    decomp.chunks.length - 1,
    Math.ceil(decomp.chunks.length / 2)
  );
  console.log(decomp.chunks, init);
  show_chunks(decomp.chunks[init][1]);

  panel.webview.onDidReceiveMessage((i) => {
    show_chunks((decomp as Decomposition).chunks[i][1]);
  });
};

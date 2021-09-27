import * as vscode from "vscode";
import { log, show_error, CallFlowistry, to_vsc_range } from "./vsc_utils";
import { Effects, Message, Range, SelectedSlice } from "./types";
import {
  highlight_ranges,
  select_type,
  highlight_type,
  hide_type,
  invert_ranges,
  highlight_slice,
} from "./slicing";
import _ from "lodash";

export let effects = async (
  context: vscode.ExtensionContext,
  call_flowistry: CallFlowistry
) => {
  let active_editor = vscode.window.activeTextEditor;
  if (!active_editor) {
    return;
  }

  let doc = active_editor.document;
  let selection = active_editor.selection;

  let range_to_text = (range: Range): string =>
    doc.getText(to_vsc_range(range, doc));

  try {
    let cmd = `effects ${doc.fileName} ${doc.offsetAt(selection.anchor)}`;
    let effects_maybe = await call_flowistry<Effects>(cmd);
    if (effects_maybe === null) {
      return;
    }
    let effects: Effects = effects_maybe;

    let body_range = effects.body_span;

    let arg_strs = effects.args_effects.map(([arg, effects]) => {
      return {
        arg,
        effects: effects.map((effect) => range_to_text(effect.effect)),
      };
    });
    let ret_strs = effects.returns.map((effect) =>
      range_to_text(effect.effect)
    );

    let ext_dir = vscode.Uri.joinPath(context.extensionUri, "out");
    const panel = vscode.window.createWebviewPanel(
      "flowistry.effects",
      `Flowistry: effects of ${effects.fn_name}`,
      vscode.ViewColumn.Beside,
      {
        enableScripts: true,
        localResourceRoots: [ext_dir],
      }
    );
    let webview = panel.webview;

    let js_path = vscode.Uri.joinPath(ext_dir, "effects_page.js");
    let js_uri = js_path.with({ scheme: "vscode-resource" });

    let css_path = vscode.Uri.joinPath(ext_dir, "extension.css");
    let css_uri = webview.asWebviewUri(css_path);

    let csp_source = webview.cspSource;
    let nonce = "foobar";

    webview.html = `
<!DOCTYPE html>
<html>
<head>
  <link rel="stylesheet" href="${css_uri}" />
</head>          
<body class="">
  <div id="app"></div>
  <script nonce="${nonce}" src="${js_uri}"></script>
</body>        
</html>        
`;
    webview.onDidReceiveMessage(
      (message: Message) => {
        if (message.type === "click") {
          let data: SelectedSlice = message.data;
          let effect;
          if (data.type === "ret") {
            effect = effects.returns[data.index];
          } else if (data.type === "arg") {
            let [_1, arg_effects] = effects.args_effects[data.arg_index];
            effect = arg_effects[data.effect_index];
          } else {
            throw new Error("Unimplemented");
          }

          let range = to_vsc_range(effect.effect, doc);
          active_editor!.revealRange(
            range,
            vscode.TextEditorRevealType.InCenterIfOutsideViewport
          );
          highlight_slice(
            active_editor!,
            body_range,
            [effect.effect],
            effect.slice
          );
          highlight_ranges(effect.unique, active_editor!, highlight_type);
        }
      },
      null,
      []
    );

    let message: Message = {
      type: "input",
      data: { arg_strs, ret_strs, fn_name: effects.fn_name },
    };
    webview.postMessage(message);
  } catch (exc: any) {
    log("ERROR", exc);
    show_error(exc);
  }
};

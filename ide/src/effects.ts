import * as vscode from "vscode";
import { log, show_error, CallFlowistry, to_vsc_range } from "./vsc_utils";
import { Effects, Message, Range } from "./types";
import {
  highlight_ranges,
  select_type,
  highlight_type,
  hide_type,
} from "./slicing";
import _ from "lodash";

export let invert_ranges = (container: Range, pieces: Range[]): Range[] => {
  let filename = container.filename;
  let pieces_sorted = _.sortBy(pieces, (r) => r.start);

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
    filename
  });

  return new_ranges;
};

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
    let stdout = await call_flowistry(cmd);
    log(stdout);
    let lines = stdout.split("\n");
    let last_line = lines[lines.length - 1];
    let effects: Effects = JSON.parse(last_line);
    let body_range = effects.body_span;

    let args = Object.keys(effects.args_effects);
    args.sort();

    let arg_strs = args.map((arg) => {
      let arg_effects = effects.args_effects[arg].map((effect) =>
        range_to_text(effect.effect)
      );
      return {
        arg,
        effects: arg_effects,
      };
    });
    let ret_strs = effects.returns.map((effect) =>
      range_to_text(effect.effect)
    );

    let ext_dir = vscode.Uri.joinPath(context.extensionUri, "out");
    const panel = vscode.window.createWebviewPanel(
      "flowistry",
      "Flowistry",
      vscode.ViewColumn.Beside,
      {
        enableScripts: true,
        localResourceRoots: [ext_dir],
      }
    );
    let webview = panel.webview;

    let js_path = vscode.Uri.joinPath(ext_dir, "effects_page.js");
    let js_uri = js_path.with({ scheme: "vscode-resource" });
    let csp_source = webview.cspSource;
    let nonce = "foobar";
    webview.html = `
<!DOCTYPE html>
<html>
<head></head>          
<body>
<div id="app"></div>
<script nonce="${nonce}" src="${js_uri}"></script>
</body>        
</html>        
`;
    webview.onDidReceiveMessage(
      (message: Message) => {
        if (message.type == "click") {
          let type = message.data.type;
          if (type === "ret") {
            let { index } = message.data;
            let effect = effects.returns[index];
            highlight_ranges(effect.slice, active_editor!, highlight_type);
          } else if (type === "arg") {
            let { arg_index, effect_index } = message.data;
            let arg = args[arg_index];
            let effect = effects.args_effects[arg][effect_index];

            let range = to_vsc_range(effect.effect, doc);
            active_editor!.revealRange(
              range,
              vscode.TextEditorRevealType.InCenter
            );
            highlight_ranges([effect.effect], active_editor!, select_type);

            highlight_ranges(
              invert_ranges(body_range, effect.slice),
              active_editor!,
              hide_type
            );
          }
        }
      },
      null,
      []
    );

    let message: Message = {
      type: "input",
      data: { arg_strs, ret_strs },
    };
    webview.postMessage(message);
  } catch (exc) {
    log("ERROR", exc);
    show_error(exc);
  }
};

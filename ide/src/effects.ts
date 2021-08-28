import * as vscode from "vscode";
import { log, show_error, CallFlowistry } from "./vsc_utils";
import { Effects, Message } from "./types";

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

  try {
    let cmd = `effects yeehaw`;
    let stdout = await call_flowistry(cmd);
    log(stdout);
    let lines = stdout.split("\n");
    let last_line = lines[lines.length - 1];
    let effects: Effects = JSON.parse(last_line);

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

    let js_path = vscode.Uri.joinPath(ext_dir, "effects.js");
    let js_uri = js_path.with({ scheme: "vscode-resource" });
    let csp_source = webview.cspSource;
    let nonce = "foobar";
    webview.html = `
<!DOCTYPE html>
<html>
<head>
<meta http-equiv="Content-Security-Policy" content="default-src 'none'; style-src ${csp_source}; img-src ${csp_source} https:; script-src 'nonce-${nonce}';">
</head>          
<body>
<div id="app"></div>
<script nonce="${nonce}" src="${js_uri}"></script>
</body>        
</html>        
`;
    webview.onDidReceiveMessage(
      (message: Message) => {
        log(JSON.stringify(message));
      },
      null,
      []
    );

    let message: Message = {
      type: "input",
      data: effects,
    };
    webview.postMessage(message);
  } catch (exc) {
    log("ERROR", exc);
    show_error(exc);
  }
};

import * as vscode from "vscode";
import _ from "lodash";
import { CallFlowistry, log, show_error } from "./vsc_utils";

import { decompose } from "./decompose";
import { focus, focus_mark, focus_unmark, focus_select } from "./focus";
import { setup } from "./setup";

import "./app.scss";

export async function activate(context: vscode.ExtensionContext) {
  log("Activating...");

  try {
    let call_flowistry = await setup(context);
    if (call_flowistry === null) {
      return;
    }

    let commands: [string, (_f: CallFlowistry) => void][] = [
      ["focus", focus],
      ["focus_mark", focus_mark],
      ["focus_unmark", focus_unmark],
      ["focus_select", focus_select],
      ["decompose", decompose],
    ];

    commands.forEach(([name, func]) => {
      let disposable = vscode.commands.registerCommand(
        `flowistry.${name}`,
        () => {
          try {
            func(call_flowistry!);
          } catch (exc: any) {
            log("ERROR", exc);
            show_error(exc);
          }
        }
      );
      context.subscriptions.push(disposable);
    });
  } catch (e: any) {
    show_error(e.toString());
  }

  log("flowistry is activated");
}

export function deactivate() {}

import * as vscode from "vscode";
import _ from "lodash";
import {
  CallFlowistry,
  FlowistryErrorDocument,
  last_error,
  log,
  show_error,
} from "./vsc_utils";

import { decompose } from "./decompose";
import { FocusMode } from "./focus";
import { setup } from "./setup";

import "./app.scss";

export let flowistry_status_bar_item: vscode.StatusBarItem;
export const tdcp = new FlowistryErrorDocument();

export async function activate(context: vscode.ExtensionContext) {
  log("Activating...");

  try {
    flowistry_status_bar_item = vscode.window.createStatusBarItem();
    context.subscriptions.push(flowistry_status_bar_item);
    flowistry_status_bar_item.show();

    context.subscriptions.push(
      vscode.workspace.registerTextDocumentContentProvider("flowistry", tdcp)
    );

    let call_flowistry = await setup(context);
    if (call_flowistry === null) {
      return;
    }

    let focus = new FocusMode(call_flowistry, flowistry_status_bar_item);

    let commands: [string, (_f: CallFlowistry) => void][] = [
      ["focus", focus.focus.bind(focus)],
      ["focus_mark", focus.focus_mark.bind(focus)],
      ["focus_unmark", focus.focus_unmark.bind(focus)],
      ["focus_select", focus.focus_select.bind(focus)],
      ["decompose", decompose],
      ["last_error", last_error.bind(context)],
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

import _ from "lodash";
import * as vscode from "vscode";

import "./app.scss";
import { decompose } from "./decompose";
import { ErrorPane, last_error, show_error_dialog } from "./errors";
import { FocusMode } from "./focus";
import { log } from "./logging";
import { CallFlowistry, setup } from "./setup";
import { StatusBar } from "./status_bar";

export let globals: {
  status_bar: StatusBar;
  error_pane: ErrorPane;
  call_flowistry: CallFlowistry;
};

export async function activate(context: vscode.ExtensionContext) {
  log("Activating...");

  try {
    globals = {
      status_bar: new StatusBar(context),
      error_pane: await ErrorPane.load(context),
      call_flowistry: () => {
        throw Error(`Unreachable`);
      },
    };

    let call_flowistry = await setup(context);
    if (call_flowistry === null) {
      return;
    }

    await call_flowistry("preload", true);

    globals.call_flowistry = call_flowistry;

    let focus = new FocusMode();
    let commands: [string, () => Promise<void>][] = [
      ...focus.commands(),
      ["decompose", decompose],
      ["last_error", () => last_error(context)],
    ];

    commands.forEach(([name, func]) => {
      let disposable = vscode.commands.registerCommand(
        `flowistry.${name}`,
        async () => {
          try {
            await func();
          } catch (exc: any) {
            log("ERROR", exc);
            show_error_dialog(exc);
          }
        }
      );
      context.subscriptions.push(disposable);
    });
  } catch (e: any) {
    show_error_dialog(e.toString());
  }

  log("flowistry is activated");
}

export function deactivate() {}

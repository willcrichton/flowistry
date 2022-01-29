import * as vscode from "vscode";
import _ from "lodash";
import { log, show_error } from "./vsc_utils";

import { slice } from "./slicing";
import { find_mutations } from "./mutations";
import { effects } from "./effects";
import { decompose } from "./decompose";
import { focus } from "./focus";
import { setup } from "./setup";

import "./app.scss";

export async function activate(context: vscode.ExtensionContext) {
  log("Activating...");

  try {
    let call_flowistry = await setup(context);
    if (call_flowistry === null) {
      return;
    }

    let register_with_opts = (name: string, f: () => void) => {
      let disposable = vscode.commands.registerCommand(`flowistry.${name}`, f);
      context.subscriptions.push(disposable);
    };

    ["backward", "forward"].forEach((direction: any) => {
      ["highlight", "select"].forEach((type: any) => {
        register_with_opts(`${direction}_${type}`, () =>
          slice(call_flowistry!, direction, type)
        );
      });
    });

    register_with_opts("highlight_mutations", () =>
      find_mutations(call_flowistry!, "highlight")
    );
    register_with_opts("select_mutations", () =>
      find_mutations(call_flowistry!, "select")
    );
    register_with_opts("backward_highlight_recurse", () =>
      slice(call_flowistry!, "backward", "highlight", "--contextmode Recurse")
    );
    register_with_opts("backward_highlight_ignoremut", () =>
      slice(
        call_flowistry!,
        "backward",
        "highlight",
        "--mutabilitymode IgnoreMut"
      )
    );

    register_with_opts("effects", () => effects(context, call_flowistry!));

    register_with_opts("decompose", () => decompose(call_flowistry!));

    register_with_opts("focus", () => focus(call_flowistry!));
  } catch (e: any) {
    show_error(e.toString());
  }

  log("flowistry is activated");
}

export function deactivate() {}

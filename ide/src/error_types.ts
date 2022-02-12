import vscode from "vscode";
import { log, show_error } from "./vsc_utils";
import { tdcp } from "./extension";

export interface FlowistryError extends Error {
  show(): Promise<void>;
}

export class FlowistryBuildError extends Error implements FlowistryError {
  constructor(msg: string) {
    super(msg);
    log("ERROR", msg);
    Object.setPrototypeOf(this, FlowistryBuildError.prototype);
  }

  async show() {
    tdcp.contents = this.message.toString();
    tdcp.eventEmitter.fire(tdcp.uri);
    let doc = await vscode.workspace.openTextDocument(tdcp.uri);
    await vscode.window.showTextDocument(doc, vscode.ViewColumn.Beside);
  }
}

export class FlowistryRuntimeError extends Error implements FlowistryError {
  constructor(msg: string) {
    super(msg);
    log("ERROR", msg);
    Object.setPrototypeOf(this, FlowistryRuntimeError.prototype);
  }

  async show() {
    await show_error(this.message);
  }
}

export const is_flowisty_error = (err: any): err is FlowistryError => {
  return err.show !== undefined;
};

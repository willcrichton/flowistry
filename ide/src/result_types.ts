import vscode from "vscode";
import { globals } from "./extension";
import { show_error } from "./vsc_utils";
import _ from "lodash";

export interface BuildError {
  type: "build-error";
  error: string;
}
interface AnalysisError {
  type: "analysis-error";
  error: string;
}
interface FlowistryOutput<T> {
  type: "output";
  value: T;
}
export type FlowistryResult<T> =
  | FlowistryOutput<T>
  | BuildError
  | AnalysisError;

export const show = async (error: BuildError | AnalysisError) => {
  if (error.type === "build-error") {
    let tdcp = globals.error_doc;
    tdcp.contents = error.error;
    tdcp.eventEmitter.fire(tdcp.uri);
    let doc = await vscode.workspace.openTextDocument(tdcp.uri);
    let is_visible = _.some(
      vscode.window.visibleTextEditors,
      (editor) => editor.document === doc
    );
    if (!is_visible) {
      await vscode.window.showTextDocument(doc, vscode.ViewColumn.Beside);
    }
  } else {
    await show_error(error.error);
  }
};

export let ok = <T>(value: T): FlowistryResult<T> => ({
  type: "output",
  value,
});

export const is_ok = <T>(res: FlowistryResult<T>): res is FlowistryOutput<T> =>
  res.type === "output";

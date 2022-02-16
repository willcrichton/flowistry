import vscode from "vscode";
import { tdcp } from "./extension";
import { show_error } from "./vsc_utils";

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
    tdcp.contents = error.error;
    tdcp.eventEmitter.fire(tdcp.uri);
    let doc = await vscode.workspace.openTextDocument(tdcp.uri);
    await vscode.window.showTextDocument(doc, vscode.ViewColumn.Beside);
  } else {
    await show_error(error.error);
  }
};

export const is_ok = <T>(res: FlowistryResult<T>): res is FlowistryOutput<T> =>
  res.type == "output";

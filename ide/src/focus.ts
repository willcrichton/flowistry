import * as vscode from "vscode";
import { highlight_slice, clear_ranges } from "./utils";
import { Range } from "./types";
import { CallFlowistry, to_vsc_range } from "./vsc_utils";
import _ from "lodash";
import IntervalTree from "@flatten-js/interval-tree";
import { FocusStatus, render_status_bar } from "./focus_utils";

interface PlaceInfo {
  range: Range;
  slice: Range[];
  mutations: Range[];
}
interface Focus {
  place_info: PlaceInfo[];
  body_range: Range;
  arg_range: Range;
}

type Interval = [number, number];

interface FocusState {
  mark: vscode.Selection | null;
  focus: Focus;
  ranges: IntervalTree<PlaceInfo>;
  disposable: vscode.Disposable;
}

export class FocusMode {
  status = FocusStatus.Inactive;
  state: FocusState | null = null;

  call_flowistry: CallFlowistry;
  status_bar_item: vscode.StatusBarItem;

  doc_save_callback: vscode.Disposable | undefined;
  doc_edit_callback: vscode.Disposable | undefined;

  constructor(
    call_flowistry: CallFlowistry,
    status_bar_item: vscode.StatusBarItem
  ) {
    this.call_flowistry = call_flowistry;
    this.status_bar_item = status_bar_item;
  }

  private add_watchers() {
    if (this.is_active()) {
      return;
    }

    // reinitialize focus mode state after each save
    this.doc_save_callback = vscode.workspace.onDidSaveTextDocument(
      async () => {
        await this.initialize(true);
      }
    );

    // pause rendering if there are unsaved changes in the doc
    this.doc_edit_callback = vscode.workspace.onDidChangeTextDocument(
      async (event) => {
        let active_editor = vscode.window.activeTextEditor!;

        if (event.document === active_editor.document) {
          if (active_editor.document.isDirty) {
            this.pause_rendering(FocusStatus.UnsavedChanges);
          } else {
            this.setStatus(FocusStatus.Active);
            this.render();
          }
        }
      }
    );
  }

  private dispose_watchers() {
    this.doc_save_callback?.dispose();
    this.doc_edit_callback?.dispose();
  }

  private setStatus(status: FocusStatus) {
    this.status = status;
    render_status_bar(this.status_bar_item, status);
  }

  private is_active() {
    return this.status !== FocusStatus.Inactive;
  }

  private should_render() {
    return [FocusStatus.Active, FocusStatus.Loading].includes(this.status);
  }

  private focus_subcommand =
    (f: (editor: vscode.TextEditor) => void) => async () => {
      let active_editor = vscode.window.activeTextEditor;
      if (!active_editor) {
        return;
      }

      if (!this.is_active()) {
        await this.initialize();
      }

      f(active_editor);
    };

  async initialize(hide_error: boolean = false) {
    this.add_watchers();

    let active_editor = vscode.window.activeTextEditor;
    if (!active_editor) {
      return;
    }

    let doc = active_editor.document;
    let selection = active_editor.selection;

    let cmd = `focus ${doc.fileName} ${doc.offsetAt(selection.anchor)}`;
    let focus = await this.call_flowistry<Focus>(cmd, hide_error);

    // pause rendering and add error status when program doesn't compile
    if (!focus) {
      return this.pause_rendering(FocusStatus.Error);
    }

    let ranges = new IntervalTree();
    focus.place_info.forEach((slice) => {
      ranges.insert([slice.range.start, slice.range.end], slice.slice);
    });

    let disposable = vscode.window.onDidChangeTextEditorSelection(() =>
      this.render()
    );
    this.state = { disposable, focus, ranges, mark: null };

    this.setStatus(FocusStatus.Active);
  }

  async render(select: boolean = false) {
    if (!this.state) {
      throw `Tried to render while state is invalid.`;
    }

    if (!this.is_active() || !this.should_render()) {
      return;
    }

    let active_editor = vscode.window.activeTextEditor!;
    let doc = active_editor.document;
    let { start, end } = this.state.mark || active_editor.selection;
    let query: Interval = [doc.offsetAt(start), doc.offsetAt(end)];

    let is_contained = (child: Interval, parent: Interval): boolean =>
      parent[0] <= child[0] && child[1] <= parent[1];

    let result = this.state.ranges.search(query, (v, k) => [
      [k.low, k.high],
      v,
    ]);
    let [contained, others] = _.partition(result, ([k]) =>
      is_contained(k, query)
    );

    console.log("query", query);
    console.log("result", result);

    let final;
    if (contained.length > 0) {
      final = contained;
      console.log("contained", contained);
    } else {
      let [containing, adjacent] = _.partition(others, ([k]) =>
        is_contained(query, k)
      );
      containing = _.sortBy(containing, ([k]) => k[1] - k[0]);
      final = adjacent.concat(containing.slice(0, 1));
      console.log("adjacent", adjacent);
      console.log("containing", containing);
    }

    let seeds = final.map(([k]) => ({
      start: k[0],
      end: k[1],
      filename: "",
    }));
    seeds = _.uniqWith(seeds, _.isEqual);
    let slice = final.map(([_k, v]) => v).flat();

    if (seeds.length > 0) {
      if (select) {
        active_editor.selections = slice.map((range) => {
          let vsc_range = to_vsc_range(range, doc);
          return new vscode.Selection(vsc_range.start, vsc_range.end);
        });
      } else {
        highlight_slice(
          active_editor,
          [this.state.focus.body_range, this.state.focus.arg_range],
          seeds,
          slice
        );
      }
    } else {
      clear_ranges(active_editor);
    }

    this.setStatus(FocusStatus.Active);
  }

  focus_mark = this.focus_subcommand((editor) => {
    this.state!.mark = editor.selection;
    this.render();
  });

  focus_unmark = this.focus_subcommand(() => {
    this.state!.mark = null;
    this.render();
  });

  focus_select = this.focus_subcommand(() => {
    this.render(true);
  });

  private pause_rendering(reason: FocusStatus) {
    clear_ranges(vscode.window.activeTextEditor!);
    this.setStatus(reason);
  }

  private uninitialize() {
    this.pause_rendering(FocusStatus.Inactive);
    this.dispose_watchers();

    this.state = null;
  }

  async focus() {
    if (this.is_active()) {
      this.uninitialize();
    } else {
      await this.initialize();
      this.render();
    }
  }
}

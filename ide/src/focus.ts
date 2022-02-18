import * as vscode from "vscode";
import { highlight_slice, clear_ranges } from "./utils";
import { Range } from "./types";
import { to_vsc_range } from "./vsc_utils";
import _ from "lodash";
import IntervalTree from "@flatten-js/interval-tree";
import { StatusBarState } from "./status_bar";
import { FlowistryResult, is_ok, ok, show } from "./result_types";
import { globals } from "./extension";

interface Spans {
  spans: Range[];
}

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

class FocusBodyState {
  mark: vscode.Selection | null;
  focus: Focus;
  places: IntervalTree<PlaceInfo>;

  constructor(focus: Focus) {
    this.mark = null;
    this.focus = focus;
    this.places = new IntervalTree();
    focus.place_info.forEach((info) => {
      this.places.insert([info.range.start, info.range.end], info);
    });
  }

  static load = async (
    editor: vscode.TextEditor
  ): Promise<FlowistryResult<FocusBodyState>> => {
    let doc = editor.document;
    let cmd = `focus ${doc.fileName} ${doc.offsetAt(editor.selection.anchor)}`;
    let focus_res = await globals.call_flowistry<Focus>(cmd);

    if (!is_ok(focus_res)) {
      return focus_res;
    }

    return ok(new FocusBodyState(focus_res.value));
  };

  private find_slice_at_selection = (
    editor: vscode.TextEditor,
    doc: vscode.TextDocument
  ): { seeds: Range[]; slice: Range[] } => {
    let { start, end } = this.mark || editor.selection;
    let query: Interval = [doc.offsetAt(start), doc.offsetAt(end)];

    let is_contained = (child: Interval, parent: Interval): boolean =>
      parent[0] <= child[0] && child[1] <= parent[1];

    let result = this.places.search(query, (v, k) => [
      [k.low, k.high],
      v.slice,
    ]);
    let [contained, others] = _.partition(result, ([k]) =>
      is_contained(k, query)
    );

    let final;
    if (contained.length > 0) {
      final = contained;
    } else {
      let [containing, adjacent] = _.partition(others, ([k]) =>
        is_contained(query, k)
      );
      containing = _.sortBy(containing, ([k]) => k[1] - k[0]);
      final = adjacent.concat(containing.slice(0, 1));
    }

    let seeds = final.map(([k]) => ({
      start: k[0],
      end: k[1],
      filename: "",
    }));
    seeds = _.uniqWith(seeds, _.isEqual);
    let slice = final.map(([_k, v]) => v).flat();

    return { seeds, slice };
  };

  render = async (editor: vscode.TextEditor, select = false) => {
    let doc = editor.document;
    let { seeds, slice } = this.find_slice_at_selection(editor, doc);

    if (seeds.length > 0) {
      if (select) {
        editor.selections = slice.map((range) => {
          let vsc_range = to_vsc_range(range, doc);
          return new vscode.Selection(vsc_range.start, vsc_range.end);
        });
      } else {
        highlight_slice(
          editor,
          [this.focus.body_range, this.focus.arg_range],
          seeds,
          slice
        );
      }
    } else {
      clear_ranges(editor);
    }
  };
}

interface FocusDocumentState {
  spans: Range[];
  bodies: IntervalTree<FocusBodyState>;
}

type FocusDocumentResult = FocusDocumentState | "error" | "unsaved";

type FocusState = Map<string, FocusDocumentResult>;

/**
 * TODO(will): explain the new state machine
 * 
 * also TODO(will): test out edge cases of the state machine. edit, save, change to new doc, disable, enable, ...
 * i noticed an error "Selection did not map to a body"
 * 
 * need a consistent philosophy on:
 *    when/how to show errors, 
 *    how to keep status bar in sync
 */
export class FocusMode {
  active: boolean = false;
  state: FocusState = new Map();

  doc_save_callback?: vscode.Disposable;
  doc_edit_callback?: vscode.Disposable;
  selection_change_callback?: vscode.Disposable;

  private set_status = (status: StatusBarState) => {
    globals.status_bar.set_state(status);
  };

  private load_spans_for_document = async (
    doc: vscode.TextDocument
  ): Promise<FlowistryResult<FocusDocumentState>> => {
    let cmd = `spans ${doc.fileName}`;
    let spans = await globals.call_flowistry<Spans>(cmd);
    if (!is_ok(spans)) {
      return spans;
    }

    return ok({ spans: spans.value.spans, bodies: new IntervalTree() });
  };

  private get_doc_state = async (
    editor: vscode.TextEditor
  ): Promise<FocusDocumentResult> => {
    let doc = editor.document;
    let filename = doc.fileName;
    let doc_state_opt = this.state.get(filename);
    if (!doc_state_opt || (doc_state_opt === "unsaved" && !doc.isDirty)) {
      let doc_state = await this.load_spans_for_document(doc);
      if (is_ok(doc_state)) {
        this.state.set(filename, doc_state.value);
      } else {
        this.state.set(filename, "error");
      }
    }

    return this.state.get(filename)!;
  };

  private get_body_state = async (
    editor: vscode.TextEditor,
    doc_state: FocusDocumentState
  ): Promise<FocusBodyState | null> => {
    let doc = editor.document;
    let { start, end } = editor.selection;
    let query: Interval = [doc.offsetAt(start), doc.offsetAt(end)];
    let overlapping_bodies = doc_state.bodies.search(query, (v, k) => [
      [k.low, k.high],
      v,
    ]);

    let body_state;
    if (overlapping_bodies.length === 0) {
      let body_state_res = await FocusBodyState.load(editor);
      if (!is_ok(body_state_res)) {
        show(body_state_res);
        this.set_status("error");
        this.state.set(doc.fileName, "error");
        return null;
      }

      body_state = body_state_res.value;
      let range: Interval = [
        body_state.focus.arg_range.start,
        body_state.focus.body_range.end,
      ];
      doc_state.bodies.insert(range, body_state);
    } else {
      body_state = overlapping_bodies[0][1];
    }

    return body_state;
  };

  private update_slice = async () => {
    let editor = vscode.window.activeTextEditor;
    if (!editor) {
      return;
    }

    if (!this.active) {
      this.set_status("idle");
      clear_ranges(editor);
      return;
    }

    let doc_state = await this.get_doc_state(editor);
    if (doc_state === "error" || doc_state === "unsaved") {
      this.set_status(doc_state);
      clear_ranges(editor);
      return;
    }

    let body_state = await this.get_body_state(editor, doc_state);
    if (body_state === null) {
      return;
    }

    this.set_status("active");
    body_state.render(editor);
  };

  private register_callbacks() {
    // rerender when the user's selection changes
    this.selection_change_callback =
      vscode.window.onDidChangeTextEditorSelection(this.update_slice);

    // pause rendering if there are unsaved changes in the doc
    this.doc_edit_callback = vscode.workspace.onDidChangeTextDocument(
      (event) => {
        let editor = vscode.window.activeTextEditor!;
        let doc = editor.document;
        if (event.document === doc && doc.isDirty) {
          this.state.set(doc.fileName, "unsaved");
          this.update_slice();
        }
      }
    );

    // reinitialize focus mode state after each save
    this.doc_save_callback = vscode.workspace.onDidSaveTextDocument(
      this.update_slice
    );
  }

  private dispose_callbacks = () => {
    this.selection_change_callback?.dispose();
    this.doc_save_callback?.dispose();
    this.doc_edit_callback?.dispose();
  };

  commands = (): [string, () => Promise<void>][] => [
    ["focus", this.focus],
    ["focus_mark", this.focus_mark],
    ["focus_unmark", this.focus_unmark],
    ["focus_select", this.focus_select],
  ];

  private focus_subcommand =
    (f: (editor: vscode.TextEditor) => void) => async () => {
      let active_editor = vscode.window.activeTextEditor;
      if (!active_editor) {
        return;
      }

      f(active_editor);
    };

  focus_mark = this.focus_subcommand(async (editor) => {
    let doc_state = await this.get_doc_state(editor);
    if (doc_state === "unsaved" || doc_state === "error") {
      return;
    }

    let body_state = await this.get_body_state(editor, doc_state);
    if (body_state === null) {
      return;
    }

    body_state.mark = editor.selection;

    this.update_slice();
  });

  focus_unmark = this.focus_subcommand(async (editor) => {
    let doc_state = await this.get_doc_state(editor);
    if (doc_state === "unsaved" || doc_state === "error") {
      return;
    }

    let body_state = await this.get_body_state(editor, doc_state);
    if (body_state === null) {
      return;
    }

    body_state.mark = null;
  });

  focus_select = this.focus_subcommand(() => {
    // TODO
    // this.render_slice(true);
  });

  focus = async () => {
    this.active = !this.active;
    if (this.active) {
      this.register_callbacks();
    } else {
      this.dispose_callbacks();
    }
    this.update_slice();
  };
}

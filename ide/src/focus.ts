import * as vscode from "vscode";
import _ from "lodash";

import { highlight_slice, clear_ranges, Cell } from "./utils";
import { Range } from "./types";
import { to_vsc_range } from "./vsc_utils";
import { RangeTree } from "./range_tree";
import { StatusBarState } from "./status_bar";
import {
  FlowistryResult,
  hide_error,
  is_ok,
  ok,
  show_error,
} from "./result_types";
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

class FocusBodyState {
  mark: vscode.Selection | null;
  focus: Focus;
  places: RangeTree<PlaceInfo>;

  constructor(focus: Focus) {
    this.mark = null;
    this.focus = focus;
    this.places = new RangeTree(
      focus.place_info.map((info) => ({ range: info.range, value: info }))
    );
  }

  static load = async (
    doc: vscode.TextDocument,
    selection: vscode.Selection
  ): Promise<FlowistryResult<FocusBodyState>> => {
    let cmd = `focus ${doc.fileName} ${doc.offsetAt(selection.anchor)}`;
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
    let query = this.places.selection_to_interval(
      doc,
      this.mark || editor.selection
    );
    let result = this.places.search(query);
    let sliced;
    if (result.contained.length > 0) {
      sliced = result.contained;
    } else {
      let first_containing = result.containing.slice(0, 1);
      sliced = result.overlapping.concat(first_containing);
    }

    let seeds = sliced.map(({ range }) => range);
    seeds = _.uniqWith(seeds, _.isEqual);
    let slice = sliced.map(({ value }) => value.slice).flat();

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

class FocusDocumentState {
  bodies: RangeTree<Cell<FlowistryResult<FocusBodyState> | null>>;
  editor: vscode.TextEditor;

  constructor(editor: vscode.TextEditor, spans: Spans) {
    this.editor = editor;
    this.bodies = new RangeTree(
      spans.spans.map((range) => ({ range, value: new Cell(null) }))
    );
  }

  static load = async (
    editor: vscode.TextEditor
  ): Promise<FlowistryResult<FocusDocumentState>> => {
    let cmd = `spans ${editor.document.fileName}`;
    let spans = await globals.call_flowistry<Spans>(cmd);
    if (!is_ok(spans)) {
      return spans;
    }

    return ok(new FocusDocumentState(editor, spans.value));
  };

  clear_ranges = () => {
    clear_ranges(this.editor);
  };

  on_change_selection = async (
    selection: vscode.Selection
  ): Promise<FlowistryResult<null>> => {
    let body_state_res = await this.get_body_state(selection);
    if (body_state_res === null) {
      return ok(null);
    }

    if (!is_ok(body_state_res)) {
      return body_state_res;
    }

    let body_state = body_state_res.value;
    body_state.render(this.editor);
    return ok(null);
  };

  get_body_state = async (
    selection: vscode.Selection
  ): Promise<FlowistryResult<FocusBodyState> | null> => {
    // Find all bodies that contain the user's selection.
    let result = this.bodies.search(
      this.bodies.selection_to_interval(this.editor.document, selection)
    );

    // If the user hasn't selected a body, then return null
    // to indicate that nothing should be done.
    if (result.containing.length === 0) {
      return null;
    }

    // If the user has selected a body, get its FocusBodyState.
    let body = result.containing[0].value;
    if (body.get() === null) {
      let body_state_res = await FocusBodyState.load(
        this.editor.document,
        selection
      );
      body.set(body_state_res);
      return body_state_res;
    } else {
      return body.get();
    }
  };
}

/**
 * TODO(will): explain the new state machine
 *
 * need to implement:
 *    closing build errors after save
 *    mark/unmark/select functionality
 *    maybe show progress bar after 5s?
 */
export class FocusMode {
  mode: StatusBarState = "idle";
  state: Map<string, FocusDocumentState> = new Map();

  doc_save_callback?: vscode.Disposable;
  doc_edit_callback?: vscode.Disposable;
  selection_change_callback?: vscode.Disposable;

  constructor() {
    // pause rendering if there are unsaved changes in the doc
    this.doc_edit_callback = vscode.workspace.onDidChangeTextDocument(
      (event) => {
        if (this.mode === "idle") {
          return;
        }

        let editor = vscode.window.activeTextEditor!;
        let doc = editor.document;
        if (event.document === doc && doc.isDirty) {
          this.set_mode("unsaved");
          this.clear_ranges();
          this.state.clear();
        }
      }
    );

    // reinitialize focus mode state after each save
    this.doc_save_callback = vscode.workspace.onDidSaveTextDocument(() => {
      if (this.mode === "idle") {
        return;
      }

      this.set_mode("active");
      this.update_slice();
    });
  }

  private set_mode = (mode: StatusBarState) => {
    this.mode = mode;
    globals.status_bar.set_state(mode);
  };

  private clear_ranges = () => {
    this.state.forEach((state) => state.clear_ranges());
  };

  private update_slice = async () => {
    let editor = vscode.window.activeTextEditor;
    if (!editor) {
      return;
    }

    if (
      this.mode === "idle" ||
      this.mode === "unsaved" ||
      this.mode === "loading"
    ) {
      return;
    }

    this.set_mode("loading");

    let filename = editor.document.fileName;
    let doc_state_opt = this.state.get(filename);
    if (!doc_state_opt) {
      let doc_state_res = await FocusDocumentState.load(editor);
      if (is_ok(doc_state_res)) {
        this.state.set(filename, doc_state_res.value);
      } else {
        await show_error(doc_state_res);
        this.set_mode("error");
        return;
      }
    }

    let doc_state = this.state.get(filename)!;
    let result = await doc_state.on_change_selection(editor.selection);
    if (!is_ok(result)) {
      await show_error(result);
      this.set_mode("error");
    } else {
      await hide_error();
      this.set_mode("active");
    }
  };

  private register_callbacks() {
    // rerender when the user's selection changes
    this.selection_change_callback =
      vscode.window.onDidChangeTextEditorSelection(this.update_slice);
  }

  private dispose_callbacks = () => {
    this.selection_change_callback?.dispose();
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
    // let doc_state = await this.get_doc_state(editor);
    // if (doc_state === "unsaved" || doc_state === "error") {
    //   return;
    // }
    // let body_state = await this.get_body_state(editor, doc_state);
    // if (body_state === null) {
    //   return;
    // }
    // body_state.mark = editor.selection;
    // this.update_slice();
  });

  focus_unmark = this.focus_subcommand(async (editor) => {
    // let doc_state = await this.get_doc_state(editor);
    // if (doc_state === "unsaved" || doc_state === "error") {
    //   return;
    // }
    // let body_state = await this.get_body_state(editor, doc_state);
    // if (body_state === null) {
    //   return;
    // }
    // body_state.mark = null;
  });

  focus_select = this.focus_subcommand(() => {
    // TODO
    // this.render_slice(true);
  });

  focus = async () => {
    if (this.mode === "idle") {
      this.set_mode("active");
      this.register_callbacks();
      this.update_slice();
    } else {
      this.set_mode("idle");
      this.clear_ranges();
      this.dispose_callbacks();
      await hide_error();
    }
  };
}

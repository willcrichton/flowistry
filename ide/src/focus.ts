import _ from "lodash";
import * as vscode from "vscode";

import { clear_ranges, highlight_slice } from "./decorations";
import { FlowistryResult, hide_error, is_ok, ok, show_error } from "./errors";
import { globals } from "./extension";
import { Range, RangeTree, to_vsc_range } from "./range";
import { StatusBarState } from "./status_bar";

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
  arg_range?: Range;
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
        let containers = [this.focus.body_range];
        if (this.focus.arg_range) {
          containers.push(this.focus.arg_range);
        }
        highlight_slice(editor, containers, seeds, slice);
      }
    } else {
      clear_ranges(editor);
    }
  };
}

// Ensure a value can be changed by-reference
export class Cell<T> {
  t: T;
  constructor(t: T) {
    this.t = t;
  }
  set(t: T) {
    this.t = t;
  }
  get(): T {
    return this.t;
  }
}

class FocusDocumentState {
  bodies: RangeTree<Cell<FlowistryResult<FocusBodyState> | null>>;

  // NOTE: a previous version of this code tried to save a reference to
  // vscode.TextEditor to reduce API complexity. But those references were
  // seemingly invalidated after changing documents, so the editor must be
  // passed in anew each time.
  constructor(spans: Spans) {
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

    return ok(new FocusDocumentState(spans.value));
  };

  on_change_selection = async (
    editor: vscode.TextEditor
  ): Promise<FlowistryResult<null>> => {
    let body_state_res = await this.get_body_state(editor);
    if (body_state_res === null) {
      return ok(null);
    }

    if (!is_ok(body_state_res)) {
      return body_state_res;
    }

    let body_state = body_state_res.value;
    body_state.render(editor);
    return ok(null);
  };

  get_body_state = async (
    editor: vscode.TextEditor
  ): Promise<FlowistryResult<FocusBodyState> | null> => {
    // Find all bodies that contain the user's selection.
    let result = this.bodies.search(
      this.bodies.selection_to_interval(editor.document, editor.selection)
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
        editor.document,
        editor.selection
      );
      body.set(body_state_res);
      return body_state_res;
    } else {
      return body.get();
    }
  };

  set_mark = async (
    editor: vscode.TextEditor
  ): Promise<FlowistryResult<null>> => {
    let body_state_res = await this.get_body_state(editor);
    if (body_state_res === null) {
      return ok(null);
    } else if (!is_ok(body_state_res)) {
      return body_state_res;
    }

    body_state_res.value.mark = editor.selection;
    return ok(null);
  };

  unset_mark = async (
    editor: vscode.TextEditor
  ): Promise<FlowistryResult<null>> => {
    let body_state_res = await this.get_body_state(editor);
    if (body_state_res === null) {
      return ok(null);
    } else if (!is_ok(body_state_res)) {
      return body_state_res;
    }

    body_state_res.value.mark = null;
    return ok(null);
  };

  select = async (
    editor: vscode.TextEditor
  ): Promise<FlowistryResult<null>> => {
    let body_state_res = await this.get_body_state(editor);
    if (body_state_res === null) {
      return ok(null);
    } else if (!is_ok(body_state_res)) {
      return body_state_res;
    }

    body_state_res.value.render(editor, true);
    return ok(null);
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
        let editor = vscode.window.activeTextEditor!;
        let doc = editor.document;

        if (editor.document.languageId !== "rust") {
          return;
        }

        if (event.document === doc && doc.isDirty) {
          this.state.clear();

          if (this.mode !== "idle") {
            this.set_mode("unsaved");
            this.clear_ranges();
          }
        }
      }
    );

    // reinitialize focus mode state after each save
    this.doc_save_callback = vscode.workspace.onDidSaveTextDocument(() => {
      let editor = vscode.window.activeTextEditor!;

      if (editor.document.languageId !== "rust") {
        return;
      }

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
    let editor = vscode.window.activeTextEditor;
    if (editor) {
      clear_ranges(editor);
    }
  };

  private get_doc_state = async (
    editor: vscode.TextEditor
  ): Promise<FocusDocumentState | null> => {
    if (this.mode !== "active") {
      return null;
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
        return null;
      }
    }

    return this.state.get(filename)!;
  };

  private handle_analysis_result = async <T>(result: FlowistryResult<T>) => {
    if (!is_ok(result)) {
      await show_error(result);
      this.set_mode("error");
    } else {
      await hide_error();
      this.set_mode("active");
    }
  };

  private update_slice = async () => {
    let editor = vscode.window.activeTextEditor;
    if (!editor) {
      return null;
    }

    if (editor.document.languageId !== "rust") {
      return;
    }

    let doc_state = await this.get_doc_state(editor);
    if (doc_state === null) {
      return;
    }

    let result = await doc_state.on_change_selection(editor);
    await this.handle_analysis_result(result);
  };

  private register_callbacks() {
    // rerender when the user's selection changes
    this.selection_change_callback =
      vscode.window.onDidChangeTextEditorSelection(this.update_slice);
  }

  private dispose_callbacks = () => {
    this.selection_change_callback!.dispose();
  };

  commands = (): [string, () => Promise<void>][] => [
    ["focus", this.focus],
    ["focus_mark", this.focus_mark],
    ["focus_unmark", this.focus_unmark],
    ["focus_select", this.focus_select],
  ];

  private focus_subcommand =
    (
      f: (
        _editor: vscode.TextEditor,
        _state: FocusDocumentState
      ) => Promise<FlowistryResult<null>>
    ) =>
    async () => {
      let editor = vscode.window.activeTextEditor;
      if (!editor) {
        return;
      }

      if (editor.document.languageId !== "rust") {
        return;
      }

      if (this.mode === "idle") {
        this.set_mode("active");
        this.register_callbacks();
      }

      let doc_state = await this.get_doc_state(editor);
      if (doc_state === null) {
        return;
      }

      let result = await f(editor, doc_state);
      await this.handle_analysis_result(result);
      await this.update_slice();
    };

  focus_mark = this.focus_subcommand((editor, doc_state) =>
    doc_state.set_mark(editor)
  );
  focus_unmark = this.focus_subcommand((editor, doc_state) =>
    doc_state.unset_mark(editor)
  );
  focus_select = this.focus_subcommand((editor, doc_state) =>
    doc_state.select(editor)
  );

  focus = async () => {
    if (this.mode === "idle") {
      this.set_mode("active");
      this.register_callbacks();
      await this.update_slice();
    } else {
      this.set_mode("idle");
      this.clear_ranges();
      this.dispose_callbacks();
      await hide_error();
    }
  };
}

import IntervalTree from "@flatten-js/interval-tree";
import _ from "lodash";
import vscode from "vscode";

export interface CharPos {
  line: number;
  column: number;
}

export interface Range {
  start: CharPos;
  end: CharPos;
  filename: string;
}

export let to_vsc_range = (range: Range): vscode.Range =>
  new vscode.Range(
    range.start.line,
    range.start.column,
    range.end.line,
    range.end.column
  );

export let range_to_interval = (
  range: Range,
  doc: vscode.TextDocument
): Interval => {
  let vsc_range = to_vsc_range(range);
  return [doc.offsetAt(vsc_range.start), doc.offsetAt(vsc_range.end)];
};

export let interval_to_range = (
  interval: Interval,
  filename: string,
  doc: vscode.TextDocument
): Range => {
  let start = doc.positionAt(interval[0]);
  let end = doc.positionAt(interval[1]);
  return {
    start: { line: start.line, column: start.character },
    end: { line: end.line, column: end.character },
    filename,
  };
};

export type Interval = [number, number];

export type Ranged<T> = {
  range: Range;
  value: T;
};
export interface SearchResult<T> {
  contained: Ranged<T>[];
  containing: Ranged<T>[];
  overlapping: Ranged<T>[];
}

export class RangeTree<T> {
  tree: IntervalTree<T>;
  filename: string;

  constructor(entries: Ranged<T>[] = [], readonly doc: vscode.TextDocument) {
    this.tree = new IntervalTree();
    this.filename = entries.length > 0 ? entries[0].range.filename : "";
    entries.forEach(({ range, value }) => {
      this.insert(range, value);
    });
  }

  selection_to_interval(selection: vscode.Selection): Interval {
    return [
      this.doc.offsetAt(selection.start),
      this.doc.offsetAt(selection.end),
    ];
  }

  insert(range: Range, data: T) {
    console.log(
      "Inserting range",
      range,
      "at interval",
      range_to_interval(range, this.doc)
    );
    this.tree.insert(range_to_interval(range, this.doc), data);
  }

  search(query: Interval): SearchResult<T> {
    let result = this.tree.search(query, (v, k) => [[k.low, k.high], v]) as [
      Interval,
      T
    ][];

    let is_contained = (child: Interval, parent: Interval): boolean =>
      parent[0] <= child[0] && child[1] <= parent[1];

    let final: SearchResult<T> = {
      contained: [],
      containing: [],
      overlapping: [],
    };

    result.forEach(([interval, value]) => {
      let range = interval_to_range(interval, this.filename, this.doc);
      if (is_contained(interval, query)) {
        final.contained.push({ range, value });
      } else if (is_contained(query, interval)) {
        final.containing.push({ range, value });
      } else {
        final.overlapping.push({ range, value });
      }
    });

    ["contained", "containing", "overlapping"].forEach((k) => {
      let final_ = final as any;
      final_[k] = _.sortBy(final_[k], ({ range }) => range.end - range.start);
    });

    console.log("Querying", query, "for result", final);

    return final;
  }
}

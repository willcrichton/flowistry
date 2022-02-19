import IntervalTree from "@flatten-js/interval-tree";
import _ from "lodash";
import vscode from "vscode";

export interface Range {
  start: number;
  end: number;
  filename: string;
}

export let to_vsc_range = (
  range: Range,
  doc: vscode.TextDocument
): vscode.Range =>
  new vscode.Range(doc.positionAt(range.start), doc.positionAt(range.end));

type Interval = [number, number];

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

  constructor(entries: Ranged<T>[] = []) {
    this.tree = new IntervalTree();
    entries.forEach(({ range, value }) => {
      this.insert(range, value);
    });
  }

  range_to_interval(range: Range): Interval {
    return [range.start, range.end];
  }

  selection_to_interval(
    doc: vscode.TextDocument,
    selection: vscode.Selection
  ): Interval {
    return [doc.offsetAt(selection.start), doc.offsetAt(selection.end)];
  }

  insert(range: Range, data: T) {
    this.tree.insert(this.range_to_interval(range), data);
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
      let range = { start: interval[0], end: interval[1], filename: "" };
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

    return final;
  }
}

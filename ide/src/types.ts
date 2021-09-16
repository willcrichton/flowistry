export interface Range {
  start: number;
  end: number;
  filename: string;
}

export interface SliceOutput {
  ranges: Range[];
  body_span: Range;
}

export interface Effect {
  effect: Range;
  slice: Range[];
  unique: Range[];
}

export interface Effects {
  args_effects: { [arg: string]: Effect[] };
  arg_spans: { [arg: string]: Range };
  returns: Effect[];
  body_span: Range;
}

export interface Message {
  type: string;
  data: any;
}

export interface ArgSlice {
  type: "arg";
  arg_index: number;
  effect_index: number;
}

export interface RetSlice {
  type: "ret";
  index: number;
}

export type SelectedSlice = ArgSlice | RetSlice;

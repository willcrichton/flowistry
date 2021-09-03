export interface Range {
  start: number;
  end: number;
  filename: string;
}

export interface SliceOutput {
  ranges: Range[];
}

export interface Effect {
  effect: Range;
  slice: Range[];
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

import { Range } from "./range";

export interface CommandOutput {
  ranges: Range[];
  body_span: Range;
  selected_spans: Range[];
}

export interface Effect {
  effect: Range;
  slice: Range[];
  unique: Range[];
}

export interface Effects {
  args_effects: [string, Effect[]][];
  arg_spans: { [arg: string]: Range };
  returns: Effect[];
  body_span: Range;
  fn_name: string;
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

export interface EffectStrings {
  arg_strs: {
    arg: string;
    effects: string[];
  }[];
  ret_strs: string[];
  fn_name: string;
}

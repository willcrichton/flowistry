import { CallFlowistry } from "./vsc_utils";
import { display_subcmd_results } from "./utils";

export async function find_mutations(
  call_flowistry: CallFlowistry,
  type: "highlight" | "select",
  flags: string = ""
) {
  await display_subcmd_results(
    call_flowistry,
    "Mutation search",
    "find_mutations",
    type,
    flags
  );
}

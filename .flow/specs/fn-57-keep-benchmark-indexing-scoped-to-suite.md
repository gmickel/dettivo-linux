# Keep the benchmark index scoped to suite reports

## Source

Gordon requested merging and releasing Dettivo for real meeting use and explicitly waived Copilot for the two feature PRs. The final merged-code GPU benchmark completed every measurement but failed while its README reader parsed an unrelated diarization accuracy JSON file.

## Scope

Repair benchmark README discovery in the release follow-up on main. Preserve typed validation of files named as CPU/GPU benchmark suite reports and preserve all measured data and negative evidence. No runtime or model behavior changes.

## Acceptance Criteria

- **R1:** The benchmark README reads only the date-host-tier suite filenames used by the release benchmark gate. Accuracy report JSON in the same directory does not fail index generation.
- **R2:** Malformed JSON with a valid suite filename still fails explicitly. Regression tests cover both cases and the existing latest-per-host/tier behavior.

## Verification

Focused Rust regression tests, the next full benchmark filing, the final repository build/test/lint gate and the release receipt.

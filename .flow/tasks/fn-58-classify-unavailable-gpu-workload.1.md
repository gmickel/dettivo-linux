---
satisfies: [R1, R2]
---
# fn-58-classify-unavailable-gpu-workload.1 Classify recognized GPU attribution skips without excusing failures

## Description
Recognize the two GPU sampler attribution limitations in the release blocker classifier. Scope the rule to the meeting pack GPU proof step and preserve failed-row enforcement. Retain the failed gate, verify the repaired gate and publish the checked package.

## Acceptance
Both recognized counter limitations are external only on the GPU proof step; unknown reasons and failed rows still block release. Full validation and a fresh complete release gate pass.

## Done summary
The release gate now classifies the two recognized GPU attribution limitations only on the meeting pack GPU proof step. The report still says the proof was skipped and requires a per-process workload counter. Unknown reasons and failed proof rows remain blocking.

The regression reproduced the preceding unexplained blocker before the fix. Both focused tests passed; the canonical gate passed 1,015 Rust tests, 79 Qt tests and all lint/documentation checks. One unrelated binary-version test failed once, then passed alone, in all 188 crate tests and in the full rerun. The complete release gate at bf4c3bd7 passed all required steps with eight named external prerequisites and zero unexplained blockers. The failed gate remains in commit 5210d5f2.

Version 0.1.0 was published from tag commit 2c8bfa350cd39764e4976d397e2983b94487a0be after the receipt proved only docs/reports changed from the gated commit. The exact tagged package passed all 19 install checks. All four published assets were downloaded and matched their local bytes; tarball and package checksums passed. Release: https://github.com/gmickel/dettivo-linux/releases/tag/v0.1.0
## Evidence
- Commits: bf4c3bd7a9b7af67aafc047a2c24afe7c04967d5, 2c8bfa350cd39764e4976d397e2983b94487a0be
- Tests: cargo test -p dettivo-qa pack::release_blockers::tests, just build test lint, just package-bin, just install-test, just qa-release target/release, scripts/packaging/release-receipt.sh 0.1.0 2c8bfa350cd39764e4976d397e2983b94487a0be, gh release download v0.1.0; sha256sum --check SHA256SUMS; sha256sum --check dettivo-bin-0.1.0-1-x86_64.pkg.tar.zst.sha256
- PRs:

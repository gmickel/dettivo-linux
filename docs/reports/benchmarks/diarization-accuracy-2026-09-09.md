# Automatic diarization accuracy development

Gordon accepted the current native implementation for delivery to main on 2026-09-10. Two frozen candidates still failed their original held-out accuracy requirements. The second passed its fourteen development recordings but failed on fresh recordings with 17.72% pooled CPU DER and only two of seven exact speaker counts. CUDA validation was interrupted by a GPU hardware/driver availability failure. The [manifest](diarization-accuracy-2026-09-09.json) preserves both failures, frozen identities, per-recording errors/resources and the unchanged quality gate. This is delivery with known limitations, not a high-accuracy pass or production-readiness claim.

## Protocol and inputs

AMI uses [`only_words` RTTM and UEM](https://github.com/pyannote/AMI-diarization-setup/tree/67c2d539286e89f68952d5dcf83912bd9f01dfae). These references preserve annotated word gaps. VoxConverse uses [version 0.3 references](https://github.com/joonson/voxconverse/tree/24bf60be297701cd7e4ef18550c6d390c1b87365) and a UEM spanning the downloaded recording. Both use zero collar and include overlapping speech. Pooled scores weight recordings by reference speaker-seconds.

Development comprises AMI ES2002a, IS1009b and EN2001b, plus VoxConverse crixb, asxwr, bauzd and ggvel. The three AMI meetings were inspected before this task and cannot supply held-out proof. The VoxConverse development clips contain two, three, five and six speakers, including speakers with less than two seconds of total speech.

The first held-out selection comprised AMI ES2004a, TS3003a and EN2002c, plus VoxConverse blwmj, ampme, afjiv and edixl. Selection revision 2 replaced official-train TS3007a with official-test TS3003a before any held-out inference. Both external selection revisions remain available. Those AMI groups differed from the original development groups. The VoxConverse clips were selected by alphabetical order, duration and annotated count, without inspecting model outputs. This local holdout does not prove exclusion from pretrained-model training data. All seven became development data after the first round failed.

The [preceding report](diarization-real-meetings-2026-09-09.md) used manual utterance-segment intervals. Those diagnostic percentages and the word-level DER here use different references and must not be combined.

## Reproduce a score

Supply native engine JSON, the matching reference RTTM and its UEM. Reference files and audio stay outside the repository.

```sh
python3 scripts/qa/diarization_score.py hypothesis.json \
  --recording ES2002a --rttm ES2002a.rttm --uem ES2002a.uem
make test-diarization-score
```

Add `--cross-check` in an environment with `pyannote.metrics` to require agreement with that implementation. The repository scorer passed seven synthetic test methods, including optimal assignment, overlaps, empty hypotheses, disjoint evaluation regions, microsecond boundaries and malformed inputs. Independent validation compared 1,000 randomized assignment matrices with SciPy and 256 real candidate outputs with the previously cross-checked scorer. All DER components agreed within `1e-7`.

## Development evidence

Rescoring the previous native CPU automatic outputs under the strict protocol gives DER of 84.78% on ES2002a, 52.64% on IS1009b and 80.23% on EN2001b. The corresponding CUDA values are 86.07%, 50.34% and 80.07%. These are failures of automatic diarization quality on the development recordings.

The unmodified upstream Sherpa 1.13.7 executable reproduced gross overclustering on ES2002a with the same models, four CPU threads, threshold 0.5, window shift ratio 0.1 and duration settings 0.3/0.5. It emitted 108 labels in 272 turns, compared with the earlier native run's 113 labels in 270 turns. This reproduces the failure class; it does not establish byte-for-byte equivalence between those builds.

An isolated upstream probe caches local segmentation labels, embeddings, chunk-to-speaker mappings and aggregate frame counts. Replaying both ERes2Net and TitaNet ES2002a caches with unchanged settings produced exactly the same serialized turns as their inference runs. Replay takes roughly half a second per configuration and leaves personal models untouched.

Average-linkage clustering followed by centroid reassignment reduced ES2002a to four automatic labels with 21.25% strict DER and 1.74% speaker confusion using the existing ERes2Net embedding model. Aggregation calibration reduced one development candidate further to 18.91%. These are development candidates, not a selected pipeline or a held-out pass. Lower population and speech-duration filters retain brief speakers but can reintroduce extra labels on longer recordings. Higher filters can lose entire speakers in the short VoxConverse clips.

The candidate model set includes the existing Chinese ERes2Net, English ERes2Net, NVIDIA TitaNet large and WeSpeaker ResNet34-LM, using [Sherpa's supported embedding models](https://github.com/k2-fsa/sherpa-onnx/releases/tag/speaker-recongition-models). Complete, average and centroid linkage trials are retained. Optional Silero speech masking worsened DER across the tested settings on both ES2002a and IS1009b; all 80 trials remain recorded, and no VAD dependency is selected.

## Earlier candidate development

The earlier Python development candidate used English ERes2Net, average-linkage cosine threshold 0.6, at least two seconds of clean speech for training embeddings, a minimum cluster population of 1% of training embeddings rounded to even and bounded from one to fifteen, and centroid reassignment. Duration filtering and gap filling were zero. Its pooled strict DER was 11.4934%, pooled speaker confusion 2.6724%, maximum recording DER 21.0154%, and exact counts 6/7. These figures are historical development results, not the current native pipeline or held-out proof.

| Development recording | Strict DER | Automatic/reference count |
|---|---:|---:|
| ES2002a | 21.0154% | 4/4 |
| IS1009b | 13.3761% | 4/4 |
| EN2001b | 8.9489% | 4/4 |
| crixb | 6.6856% | 2/2 |
| asxwr | 1.8603% | 3/3 |
| bauzd | 8.5633% | 4/5 |
| ggvel | 19.5203% | 6/6 |

The native comparison found two pipeline differences. The shared CLI WAV reader requantizes 16-bit samples by one least-significant bit, while the production meeting protocol forwards the original PCM16 samples. The final evaluation therefore uses the production protocol. Separately, Sherpa's C wrapper replaced explicit zero duration settings with 0.3/0.5 defaults. A regression reproduced that behavior, and the checked native patch now preserves explicit zeros, including a zero clustering threshold. Earlier native v1 results are retained with their actual duration settings; they do not validate the selected candidate.

Native verification reproduced the ES2002a and IS1009b RPC turn lists exactly by running the native float32 clustering on the identical cached embeddings. The residual difference from the SciPy prototype comes from its float64 distance/linkage calculations. The native CPU automatic path scored 11.2253% pooled DER and 2.4043% confusion over seven development recordings, with six exact counts. Version 3 restores fixed-count smoothing and repeated mode switches; its automatic outputs exactly match version 2 on ES2002a and ggvel, and CPU/CUDA scores match on both clips.

The full `make build test lint` gate passed at `23cfd581`, including 898 Rust tests, 74 Qt tests, scorer/clustering regressions, lints and documentation checks. The first frozen candidate receipt has SHA-256 `e30a4f0ebbeede7b8f5668a3ce0cb9a03e4e809209ea64ed76fd40c0cb974683`. It pinned 31 model, runtime, source, scorer and runner files before the first held-out inference.

## First held-out round failed

All fourteen original-PCM RPC runs completed with valid model/runtime/provider evidence. CPU pooled DER was 16.4076%; CUDA was 16.4175%. Each provider returned the exact count on four of seven recordings, below the required 80%. Maximum recording DER stayed below 25%, pooled confusion below 5%, and the CPU/CUDA DER difference was 0.00994 percentage points. The pooled DER and count criteria failed; the other passes do not override them.

The extra AMI labels represented substantial fragments of real reference speech, not merely isolated frames. The frozen models, runs and failure are retained. ES2004a, TS3003a, EN2002c, blwmj, ampme, afjiv and edixl were explicitly retired into development before further tuning and cannot supply unseen validation for a later candidate.

## Revised candidate development validation

The revised candidate consolidates low-support centroid fragments using a support-weighted angular merge cost, while preserving well-supported similar voices. A support floor of one disables consolidation, preserving short-input acoustic separation. It uses float32 vote thresholds of 0.4 for speech and 1.2 for a second speaker, followed by 0.1-second duration filtering and gap filling. Known-count mode retains the prior clustering, vote thresholds and 0.3/0.5-second smoothing. The repeated-mode test now uses four copies of one voice separated by 350 ms, proving that automatic mode preserves four turns while known-count mode merges them into one.

Live original-PCM RPC validation reproduced the cached native-float32 output exactly on all fourteen CPU recordings. Both cohorts passed independently on both providers: original-seven pooled DER was 11.4678% on CPU and 11.4671% on CUDA, with six exact counts each; retired-seven DER was 14.9696% on each provider, with seven exact counts each. All twenty-eight runs had valid execution, library identity and provider evidence. The retired cohort's margin remains narrow.

The full `make build test lint` gate passed with exit 0 at `9971ff19`: 898 Rust tests, 74 Qt tests, scorer and clustering regressions, formatting, lints and documentation checks. Development timing overlapped other development work and is not an isolated throughput benchmark.

A fresh seven-recording set was declared before new held-out inference: AMI ES2011a, TS3004a and IB4001, plus VoxConverse djngn, aufkn, bxpwa and gzvkx. The AMI recordings come from the publisher's development split but are new local groups; the VoxConverse recordings span two, three, five and six speakers. Neither pooled development results nor retired recordings count as held-out proof. The historical CUDA throughput requirement remains separate.

## Second held-out round failed

The v4 freeze at `9971ff19` pinned 127 artifacts before inference, including models, CPU/CUDA builds, source hashes, development reports, scoring code and selection. Its SHA-256 is `fd38d49c8f620c1e19d3e55c26d144990f753e078e0eea7fe7dbd7f4947ad9e7`. All pinned artifacts remained unchanged through evaluation.

| CPU recording | Strict DER | Miss | False alarm | Confusion | Count: automatic/reference | Wall seconds | Peak RSS MiB |
|---|---:|---:|---:|---:|---:|---:|---:|
| ES2011a | 18.3494% | 6.3990% | 6.6800% | 5.2703% | 3/4 | 82.90 | 502.1 |
| TS3004a | 29.2210% | 6.3236% | 7.5019% | 15.3954% | 3/4 | 88.40 | 522.4 |
| IB4001 | 19.1089% | 5.1613% | 6.9703% | 6.9773% | 4/4 | 130.23 | 594.9 |
| djngn | 1.2488% | 0.1326% | 1.1162% | 0% | 2/2 | 15.08 | 354.2 |
| aufkn | 3.8427% | 2.8554% | 0.7614% | 0.2259% | 2/3 | 15.35 | 403.2 |
| bxpwa | 2.4172% | 1.2055% | 0.7702% | 0.4415% | 4/5 | 38.43 | 403.3 |
| gzvkx | 5.4854% | 0.9660% | 2.0242% | 2.4952% | 4/6 | 19.15 | 410.9 |

Pooled CPU DER was 17.7195%, comprising 4.8278% missed speech, 5.7458% false alarm and 7.1459% confusion. Pooled DER, maximum recording DER, pooled confusion and exact count all failed. No count exceeded twice the reference count. Low DER on several short clips did not compensate for missing speakers.

CUDA completed two valid recordings: ES2011a at 18.3548% DER and TS3004a at 29.1870%, both with three labels for four speakers. Before the paired IB4001 CUDA load, the kernel reported NVIDIA Xid 79, `GPU has fallen off the bus`. That load returned `cudaGetDeviceCount` error 999; it produced no valid score. The four remaining CUDA runs were not attempted. This infrastructure interruption is not an accuracy pass or a complete CPU/CUDA comparison; its root cause is not established.

The original runner stopped after six attempts. A separately recorded CPU-only continuation reused its unmodified frozen evaluator and completed the four remaining CPU recordings. The original failure, continuation identity and separate reports are retained. No reset, reboot or device mutation was attempted. After all seven CPU results were complete, the cohort was explicitly retired into development, bringing the total to twenty-one. CPU-only stage diagnostics may now use it; subsequent held-out proof requires another untouched set and restored CUDA availability.

## Alternative embeddings failed the development comparison

Neither TitaNet large nor WeSpeaker ResNet34-LM passed the unchanged quality criteria. Each comparison used all twenty-one development recordings, with separate results for the original seven and each retired cohort. The next seven-recording validation set remains untouched.

| Model and single combined-DER-best tested profile | Original seven DER / exact counts | First retired seven DER / exact counts | Second retired seven DER / exact counts | Combined DER / confusion / exact counts |
|---|---:|---:|---:|---:|
| TitaNet, cosine 0.5, 2 s clean training, support consolidation | 12.4100% / 3 of 7 | 17.4408% / 5 of 7 | 16.4565% / 4 of 7 | 15.1631% / 4.7222% / 12 of 21 |
| WeSpeaker, cosine 0.3, 2 s clean training, support consolidation | 25.4091% / 1 of 7 | 19.0011% / 4 of 7 | 21.9978% / 3 of 7 | 22.3436% / 11.7665% / 8 of 21 |

These rows hold one profile constant across cohorts. They do not select a different profile for each recording or dilute a failed cohort into a pooled pass. TitaNet's maximum recording DER was 22.6721%, but its count and pooled DER failures remain disqualifying. WeSpeaker's lower-threshold extension to 0.1 and 0.2 did not improve its best original-cohort DER; its usable distance scale was checked rather than assumed to match ERes2Net.

The TitaNet comparison retains 350 unique replay experiments, including 210 rows in the expanded comparison, of which seventy reuse exactly rescored original results. WeSpeaker retains 378 trials. Both models have twenty-one CPU caches with checked provenance; the original cache receipts retain their documented historical runtime-identity limitations. No replacement model was selected and no product code changed for these trials.

Subsequent inspection found a preprocessing limitation in the WeSpeaker comparison. Its downloaded ONNX metadata omits `feature_normalize_type`, so Sherpa skips time-mean subtraction for the feature bins. The graph feeds transposed features directly into its first convolution. [WeSpeaker's reference inference code](https://github.com/wenet-e2e/wespeaker/blob/fb2325c1769958ccf979667862193419c9fc0551/wespeaker/bin/infer_onnx.py) subtracts that mean and uses a Hamming window. The results above describe the downloaded model as tested; the corrected comparison below supersedes their interpretation as evidence against WeSpeaker's achievable quality.

A separate cached-mask diagnosis examined the seven second-round recordings. Replacing an insufficient clean mask with the full speaker mask would add local slots, but none of those candidate slots had the missing aufkn voice or one of the two missing gzvkx voices as its dominant reference contributor. Repeated-window attribution is diagnostic only; it does not prove that an embedding would be usable or that full-mask extraction would improve DER. No full-mask fallback was implemented.

After a machine reboot, a read-only check on 9 September at 19:24 UTC found the RTX 4090 visible again. The agent did not initiate that reboot. A subsequent native v4 CUDA run on development recording crixb completed with verified GPU activity, library identity and original-PCM protocol exchange. Its 6.297399% DER exactly matched the pre-reboot CUDA result. This single recovery check does not complete the interrupted second-round CUDA evaluation or establish new-candidate parity. The failed frozen reports remain unchanged.

## Clustering, fusion and short-window probes did not generalize

All twenty-one cached native v4 controls matched before the clustering and fusion comparisons. Forcing distinct local slots within one window into separate initial clusters, then applying the existing consolidation rule, reduced TS3004a DER from 29.2210% to 20.918%. It also split other speakers. Across twenty-one recordings, DER rose from 14.2540% to 15.9731%, confusion rose to 5.5383%, and exact counts fell from fifteen to fourteen. Every seven-recording cohort failed the candidate criteria.

Concatenating normalized ERes2Net and TitaNet embeddings also helped selected recordings without passing the broader test. All input segmentation masks and embedding-row identities matched before fusion. At cosine threshold 0.45, ERes2Net weights of 0.25, 0.5 and 0.75 produced pooled DER of 15.4394%, 15.1187% and 15.0484%, with eleven, thirteen and thirteen exact counts. Every cohort failed for every tested weight. No dual-model runtime was selected.

Shorter segmentation windows used isolated metadata copies and retained the model's receptive-field timestamp mapping. Ten-second control labels, turns and scores matched v4 exactly on all four initial recordings. Five-second windows recovered four labels on TS3004a at 19.267% DER, but lost speakers elsewhere. Two-second windows on bauzd, aufkn and gzvkx also failed to provide a general repair. Bauzd DER rose above 25%; gzvkx still returned four labels for six speakers. The two-second aufkn arm returned three labels only at the lowest clean-speech threshold, with higher DER and no newly dominant clean embedding for its brief third reference voice. These reference attributions were computed after inference and never entered clustering.

All failed probes remain available with source, model, input and runtime identities. None was promoted to a product change or evaluated on the next untouched validation set.

## Correct preprocessing improved the models but did not clear acceptance

Adding WeSpeaker's missing mean normalization to a separate model copy preserved every original graph byte. At the same 0.5 clustering threshold, three-recording pooled DER fell from 57.3604% to 14.6879%, and exact counts rose from zero to three. Matching the remaining publisher settings uses a Hamming window, frame-edge truncation and an 8 kHz upper filter-bank limit. The generic runtime instead used a Povey window, reflected edge frames and a 7.6 kHz limit.

The complete corrected-WeSpeaker comparison collected twenty-one CPU caches and scored the predeclared nine profiles. Every cache retained the old segmentation and embedding-row identities. The combined-DER-best profile used threshold 0.5 and two seconds of clean training speech:

| Cohort | DER | Confusion | Exact counts | Gate |
|---|---:|---:|---:|---|
| Original seven | 11.4060% | 2.4600% | 6/7 | Pass |
| First retired seven | 16.6868% | 4.5948% | 5/7 | Fail |
| Second retired seven | 15.9133% | 5.3265% | 3/7 | Fail |
| Combined | 14.3617% | 3.9193% | 14/21 | Fail |

No tested profile passed both retired cohorts. Correct preprocessing makes this model competitive; it does not resolve the automatic-count problem.

The same source audit found upper-frequency mismatches in TitaNet and ERes2Net, plus a frame-edge mismatch in ERes2Net. The isolated ERes2Net control reproduced the original embeddings exactly. Correcting its two settings reduced TS3004a DER from 29.2210% to 20.8408% and recovered four speakers at the existing threshold and training-duration setting. A four-recording recalibration still failed to recover every count. TitaNet's three-recording frequency-only correction reduced pooled DER from 16.9324% to 14.5468%, with two exact counts instead of zero. Its full twenty-one-recording comparison scored 126 predeclared trials; no profile passed any complete cohort. The combined-DER-best profile scored 15.9031% DER with ten exact counts. These are isolated experiments, with existing model files unchanged.

An additional clustering experiment combined the corrected older WeSpeaker encoder with publicly redistributed PLDA weights and the published VBx algorithm at fixed defaults. Ordinary assignment scored 14.0290% pooled DER with thirteen exact counts; distinct-slot assignment scored 14.2157% with the same count total. Neither passed the retired-cohort gates. This is an explicitly mixed-component experiment using v4 reconstruction. Synthetic tests found close but unequal outputs between the older and public Community-1 encoders, so it is not an exact Community-1 reproduction or evidence of that complete pipeline's quality.

Fresh extraction with the [public ONNX conversion](https://huggingface.co/altunenes/speaker-diarization-community-1-onnx) on three development recordings compared all 4,949 embedding rows against the older encoder. The worst cosine similarity was 0.999998846, but no row was byte-identical. The six subsequent VBx score/count results matched the older-encoder experiment exactly, including its incorrect five-speaker result on IS1009b. This isolates encoder identity on those three recordings; it does not prove equivalence of the complete pipelines or resolve the count failure. Model and [PLDA redistribution](https://github.com/altunenes/pyannote-rs/tree/1647adaf6044812c7210497e05d1a6c71d65f125/models/plda) receipts retain pyannote's CC-BY-4.0 attribution and the conversion provenance. Downloads were anonymous; no gated access conditions or account changes were accepted.

A subsequent predeclared calibration varied the existing VBx prior and minimum clean training duration. All eighteen profiles failed acceptance across twenty-one recordings. The unchanged control remained the combined-DER-best result; lowering the prior increased fragmentation. All forty-two control cases reproduced their intermediate arrays, label files, turns and full scores before new profiles ran. No calibration result was selected.

## Stock Community-1 also failed the count requirement

A separate environment ran unmodified pyannote-audio 4.0.7 with PyTorch and torchaudio 2.8.0, torchcodec 0.7.0, the complete publicly redistributed Community-1 weights and the published configuration. All five model/configuration hashes matched independently recorded source hashes. Each inference process had no network or reference-file access, used original PCM16 samples and returned the regular overlapping diarization output. The exclusive output was not used. Neither known speaker counts nor corpus annotations entered inference.

All twenty-one CUDA runs completed. Independent readback verified every source/model/input hash, PCM hash and strict score; raw and UEM-scored speaker counts matched on every recording. Five initial results were reused only after exact rescoring. Stock timestamps retained their floating-point precision before the scorer's microsecond rounding.

| Development cohort | DER | Confusion | Exact counts | Gate |
|---|---:|---:|---:|---|
| Original seven | 12.0099% | 3.2071% | 4/7 | Fail |
| First retired seven | 15.7798% | 2.7376% | 6/7 | Fail |
| Second retired seven | 15.3989% | 4.9943% | 3/7 | Fail |
| Combined | 14.1633% | 3.4983% | 13/21 | Fail |

Maximum recording DER was 20.7746%. The pooled DER and confusion criteria passed, but thirteen exact counts out of twenty-one did not meet the 80% requirement. This GPU reference is neither a native CPU/CUDA acceptance run nor held-out proof.

Reference-only diagnosis explains why the brief-voice cases need more than a clean-embedding threshold change. Aufkn's missing 0.96-second voice has no solo annotated speech. Gzvkx's missing 3.08-second voice is also entirely overlapped, including 1.12 seconds of three-way overlap; its other missing 2.8-second voice has only 0.8 seconds alone. These annotations were inspected after inference and were never model inputs.

## Direct-attractor probe failed too

The MIT-licensed [DiaPer release](https://github.com/BUTSpeechFIT/DiaPer/tree/3c4af3cbc8e9c1840a276cc07f2c564adcd6bb2e) offers a different mechanism, predicting speaker tracks directly with ten attractors. The bounded test used its published 16 kHz configuration and the specified checkpoint average, with automatic counts and no chunking. Only paths and device selection were overridden. An isolated loader compatibility repair cleared an executable-stack flag in a preserved-copy library; model weights and algorithm settings were unchanged.

All three complete clips ran below the process memory cap. Bauzd scored 14.0419% DER with two labels for five speakers; aufkn scored 26.7157% with three labels for three speakers; gzvkx scored 9.4371% with four labels for six speakers. Aufkn's matching count was false recovery: the extra track had zero overlap with the brief reference voice. Both brief gzvkx voices remained unmatched. This generic checkpoint was not selected or expanded to the meeting set.

A separate DiaPer memory probe released an unused attention-visualization cache after each layer. It preserved the original arithmetic and precision. All logits, probabilities, hard labels, RTTM bytes and strict scores matched exactly on the same three clips. Peak GPU process memory fell from 10,232 to 5,648 MiB on bauzd, from 3,740 to 3,134 MiB on aufkn and from 4,110 to 3,180 MiB on gzvkx. The accuracy failures remained unchanged. This external experiment did not alter the native runtime.

Earlier query-row tiling attempts failed the predeclared raw-logit tolerance on gzvkx, including a float32 retry with TF32 disabled. Their hard labels and scores matched, but the numerical gate failed and those variants were rejected. Their original outputs remain preserved. The successful cache release does not remove the full attention matrix's quadratic memory requirement or establish full-meeting feasibility.

The publisher's AMI Mix-Headset-adapted checkpoint average then ran on three complete development meetings using the same cache release and fixed automatic settings. A CPU pristine-versus-cache-release control first reproduced all logits, probabilities, labels, RTTM and scores exactly on gzvkx. This was a within-CPU comparison, not CPU/CUDA quality parity.

| Meeting | Strict DER | Automatic/reference speakers | Peak RSS | Wall time |
|---|---:|---:|---:|---:|
| ES2011a | 47.8786% | 2/4 | 15.8 GiB | 24.6 s |
| TS3004a | 39.0261% | 3/4 | 22.7 GiB | 30.2 s |
| ES2004a | 35.2685% | 3/4 | 14.1 GiB | 16.5 s |

All three CPU runs completed under a hard 32 GiB address-space cap with four threads, no swap, no GPU, no network and no reference inputs. Pooled DER was 40.7130%, confusion was 13.0621%, and no automatic count matched. These are model-quality failures. The adapted model was rejected without broader evaluation. Its published training configuration contains directory placeholders, so exact training or model-selection overlap with these already-inspected development meetings could not be established. No fresh validation recordings were used.

These AMI and VoxConverse evaluations use English speech. They do not establish multilingual accuracy or production readiness.

External raw evidence, replay probes, model hashes, negative trials, scorer cross-checks and selection revisions are staged under `/home/gordon/work/dettivo-linux-wt/_factory/diarization-accuracy-gNIbaI/`. Raw audio and hypothesis turns are not committed.

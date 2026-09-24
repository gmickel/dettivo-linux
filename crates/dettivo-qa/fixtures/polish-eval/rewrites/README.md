# Recorded rewrites for the sample set

One file per row of `../sample.jsonl`, named by the first sixteen hex characters of the SHA-256 of the trimmed transcript, the key `DETTIVO_MOCK_LLM=fixture:<dir>` replays by. Two rows are staged to fail on purpose: `sample-006` drops a protected span (the guard rejects it and the deterministic text goes in) and `sample-008` answers the dictation instead of rewriting it (a forbidden fragment the scorer counts). Regenerate with the row texts in this directory's history, or write a new `<sha>.txt` by hand.

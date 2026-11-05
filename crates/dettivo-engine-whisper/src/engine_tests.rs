use super::*;

fn token(id: i32) -> whisper_rs::WhisperTokenData {
    whisper_rs::WhisperTokenData {
        id,
        tid: 0,
        p: 1.0,
        plog: 0.0,
        pt: 0.0,
        ptsum: 0.0,
        t0: 0,
        t1: 0,
        t_dtw: 0,
        vlen: 0.0,
    }
}

/// Runs the mask over `ids` with timestamps starting at id 100.
fn masked(ids: &[i32]) -> Vec<usize> {
    let mut beg = 100;
    let tokens: Vec<_> = ids.iter().copied().map(token).collect();
    let mut logits = vec![0.0f32; 200];
    #[allow(unsafe_code)]
    // SAFETY: the pointers cover `tokens`, `logits` and `beg`, all alive here.
    unsafe {
        dtw_safe_timestamps(
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            tokens.as_ptr(),
            tokens.len() as i32,
            logits.as_mut_ptr(),
            (&raw mut beg).cast(),
        );
    }
    (0..logits.len())
        .filter(|&i| logits[i].is_infinite())
        .collect()
}

/// A decode that opened a segment may not close it before 160 ms, so
/// whisper.cpp's DTW median filter never sees a window it aborts on.
#[test]
fn early_closing_timestamps_are_masked_once_a_segment_opened() {
    assert!(masked(&[]).is_empty(), "the opening timestamp stays free");
    assert!(masked(&[5, 6]).is_empty(), "text alone opens nothing");
    assert_eq!(masked(&[100, 5]), (100..108).collect::<Vec<_>>());
}

//! Stop strings over a token stream. A stop string can arrive over
//! several tokens, so the streamed text is the accumulated text less the
//! longest suffix that could still turn into a stop string; that suffix is
//! released when a later piece rules the match out, and discarded when
//! the stop completes. The partials therefore always concatenate to the
//! final answer.

/// The byte offset of the earliest stop string in `text`, if any.
pub fn stop_at(text: &str, stop: &[String]) -> Option<usize> {
    stop.iter()
        .filter(|s| !s.is_empty())
        .filter_map(|s| text.find(s.as_str()))
        .min()
}

/// How many bytes at the end of `text` could still begin a stop string:
/// the longest suffix that is a proper prefix of one of `stop`.
pub fn held_back(text: &str, stop: &[String]) -> usize {
    stop.iter()
        .filter(|s| !s.is_empty())
        .flat_map(|s| {
            (1..s.len())
                .filter(|&k| s.is_char_boundary(k))
                .filter(|&k| text.ends_with(&s[..k]))
        })
        .max()
        .unwrap_or(0)
}

/// The generated text and how much of it has been streamed.
#[derive(Debug, Default)]
pub struct Streamed {
    text: String,
    emitted: usize,
}

impl Streamed {
    /// Appends `piece`. When a stop string completes, the text is cut
    /// before it and `true` comes back; otherwise every byte that can no
    /// longer begin a stop string is streamed through `partial`.
    pub fn push(&mut self, piece: &str, stop: &[String], partial: &mut dyn FnMut(&str)) -> bool {
        self.text.push_str(piece);
        if let Some(at) = stop_at(&self.text, stop) {
            self.text.truncate(at);
            return true;
        }
        let safe = self.text.len() - held_back(&self.text, stop);
        if safe > self.emitted {
            partial(&self.text[self.emitted..safe]);
            self.emitted = safe;
        }
        false
    }

    /// Streams whatever is still held back and returns the whole text.
    pub fn finish(self, partial: &mut dyn FnMut(&str)) -> String {
        if self.text.len() > self.emitted {
            partial(&self.text[self.emitted..]);
        }
        self.text
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stops(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| (*s).to_string()).collect()
    }

    fn stream(pieces: &[&str], stop: &[String]) -> (Vec<String>, String, bool) {
        let mut partials = Vec::new();
        let mut streamed = Streamed::default();
        let mut stopped = false;
        for piece in pieces {
            if streamed.push(piece, stop, &mut |p| partials.push(p.to_string())) {
                stopped = true;
                break;
            }
        }
        let text = streamed.finish(&mut |p| partials.push(p.to_string()));
        (partials, text, stopped)
    }

    #[test]
    fn stop_strings_cut_at_the_earliest_one() {
        let stop = stops(&["</rewrite>", "\n\n"]);
        assert_eq!(stop_at("Hello.</rewrite> more", &stop), Some(6));
        assert_eq!(stop_at("a\n\nb</rewrite>", &stop), Some(1));
        assert_eq!(stop_at("nothing here", &stop), None);
        assert_eq!(stop_at("x", &[String::new()]), None);
    }

    /// engines/F12: the release probe's stop string `3\n` streamed `3`
    /// and answered nothing; now the partials and the answer agree.
    #[test]
    fn a_stop_string_that_spans_tokens_is_never_streamed() {
        let stop = stops(&["3\n"]);
        let (partials, text, stopped) = stream(&["1", "2", "3", "\n", "4"], &stop);
        assert!(stopped);
        assert_eq!(text, "12");
        assert_eq!(partials.concat(), text);
        assert_eq!(partials, vec!["1", "2"]);

        let stop = stops(&["</rewrite>"]);
        let (partials, text, stopped) = stream(&["Hello", ".</", "rew", "rite>", " more"], &stop);
        assert!(stopped);
        assert_eq!(text, "Hello.");
        assert_eq!(partials.concat(), text);
    }

    #[test]
    fn a_held_prefix_is_released_when_the_match_fails_or_at_the_end() {
        let stop = stops(&["</rewrite>"]);
        let (partials, text, stopped) = stream(&["a <", "b> c"], &stop);
        assert!(!stopped);
        assert_eq!(text, "a <b> c");
        assert_eq!(partials, vec!["a ", "<b> c"]);

        // Ordinary end of generation flushes what was held back.
        let (partials, text, stopped) = stream(&["done", "</rew"], &stop);
        assert!(!stopped);
        assert_eq!(text, "done</rew");
        assert_eq!(partials, vec!["done", "</rew"]);
    }

    #[test]
    fn holding_back_respects_character_boundaries() {
        let stop = stops(&["…end"]);
        assert_eq!(held_back("text …", &stop), "…".len());
        assert_eq!(held_back("text …e", &stop), "…e".len());
        assert_eq!(held_back("text", &stop), 0);
        let (partials, text, stopped) = stream(&["say ", "…", "en", "d now"], &stop);
        assert!(stopped);
        assert_eq!(text, "say ");
        assert_eq!(partials.concat(), text);
    }
}

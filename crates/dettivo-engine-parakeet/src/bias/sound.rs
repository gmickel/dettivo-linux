//! How alike two spellings are: letters in common, edit distance, and a
//! rough English consonant key with a sound count, so a heard span can be
//! matched to a term it may be a mishearing of.

/// Lowercase letters and digits only: `"Sapien XT"` is `sapienxt`.
pub fn normalise(text: &str) -> String {
    text.chars()
        .filter(|c| c.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

/// Levenshtein distance over characters.
pub fn edit_distance(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let mut row: Vec<usize> = (0..=b.len()).collect();
    for (i, ca) in a.iter().enumerate() {
        let mut diagonal = row[0];
        row[0] = i + 1;
        for (j, cb) in b.iter().enumerate() {
            let next = (row[j + 1] + 1)
                .min(row[j] + 1)
                .min(diagonal + usize::from(ca != cb));
            diagonal = row[j + 1];
            row[j + 1] = next;
        }
    }
    row[b.len()]
}

/// The longest common subsequence's length.
pub fn common_letters(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let mut row = vec![0usize; b.len() + 1];
    for ca in &a {
        let mut diagonal = 0;
        for (j, cb) in b.iter().enumerate() {
            let next = if ca == cb {
                diagonal + 1
            } else {
                row[j + 1].max(row[j])
            };
            diagonal = row[j + 1];
            row[j + 1] = next;
        }
    }
    row[b.len()]
}

fn is_vowel(c: char) -> bool {
    matches!(c, 'a' | 'e' | 'i' | 'o' | 'u' | 'y')
}

/// The consonant sounds of a normalised spelling, one letter a sound
/// class (`dettivo` is `TF`), led by `A` when it starts with a vowel;
/// and how many sounds it has, vowel runs counted once.
pub fn key(word: &str) -> (String, usize) {
    let chars: Vec<char> = word.chars().filter(char::is_ascii_alphabetic).collect();
    let mut key = String::new();
    let mut sounds = 0;
    let mut i = 0;
    let push = |key: &mut String, class: char| {
        if !key.ends_with(class) {
            key.push(class);
        }
    };
    while i < chars.len() {
        let c = chars[i];
        let next = chars.get(i + 1).copied();
        let mut width = 1;
        if is_vowel(c) && !(c == 'y' && i == 0) {
            while chars.get(i + width).copied().is_some_and(is_vowel) {
                width += 1;
            }
            if i == 0 {
                key.push('A');
            } else {
                // A vowel splits doubled consonant classes (`t-t` stays `T`
                // only within one cluster).
                key.push('.');
            }
            sounds += 1;
            i += width;
            continue;
        }
        let class = match (c, next) {
            ('c' | 's', Some('h')) => {
                width = 2;
                Some('X')
            }
            ('t', Some('h')) => {
                width = 2;
                Some('0')
            }
            ('p', Some('h')) => {
                width = 2;
                Some('F')
            }
            ('g', Some('h')) => {
                width = 2;
                None
            }
            ('c', Some('k')) => {
                width = 2;
                Some('K')
            }
            ('c', Some('e' | 'i' | 'y')) => Some('S'),
            ('c' | 'g' | 'k' | 'q', _) => Some('K'),
            ('b' | 'p', _) => Some('P'),
            ('d' | 't', _) => Some('T'),
            ('f' | 'v', _) => Some('F'),
            ('s' | 'z', _) => Some('S'),
            ('j', _) => Some('J'),
            ('m', _) => Some('M'),
            ('n', _) => Some('N'),
            ('l', _) => Some('L'),
            ('r', _) => Some('R'),
            ('x', _) => {
                push(&mut key, 'K');
                Some('S')
            }
            _ => None, // h, w, y as a consonant
        };
        // A doubled letter is one sound.
        while chars.get(i + width) == Some(&c) {
            width += 1;
        }
        if let Some(class) = class {
            push(&mut key, class);
        }
        sounds += 1;
        i += width;
    }
    (key.replace('.', ""), sounds)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spellings_normalise_to_lowercase_letters_and_digits() {
        assert_eq!(normalise("Sapien XT."), "sapienxt");
        assert_eq!(normalise("Dettivo's"), "dettivos");
    }

    #[test]
    fn distances_count_edits_and_shared_letters() {
        assert_eq!(edit_distance("hyperlan", "hyprland"), 2);
        assert_eq!(edit_distance("dativo", "detivo"), 1);
        assert_eq!(common_letters("hyperlan", "hyprland"), 7);
        assert_eq!(common_letters("highland", "hyprland"), 5);
    }

    #[test]
    fn the_key_keeps_consonant_sounds_and_counts_every_sound() {
        assert_eq!(key("dettivo"), ("TTF".into(), 6));
        assert_eq!(key("itivo"), ("ATF".into(), 5));
        assert_eq!(key("omarchy"), ("AMRX".into(), 6));
        assert_eq!(key("omarhe").0, "AMR");
        assert_eq!(key("sapienxt").0, "SPNKST");
        assert_eq!(key("zapianxt").0, "SPNKST");
        assert_eq!(key("highland").0, "LNT");
        assert_eq!(key("clef"), ("KLF".into(), 4));
    }
}

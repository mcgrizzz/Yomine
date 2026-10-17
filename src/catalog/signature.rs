//! A subtitle text's MinHash signature: two signatures estimate how much of the text the two
//! files share, whatever their timing, formatting, line breaks or extra lines.

use std::collections::HashSet;

use unicode_normalization::UnicodeNormalization;

const PERMUTATIONS: usize = 128;
const SHINGLE_CHARS: usize = 5;
/// Too little text to tell episodes apart, such as a signs track.
const MIN_SHINGLES: usize = 200;
/// Stored signatures only compare with ones made the same way.
const FORMAT: u8 = 1;
/// The share of the smaller text found in the other above which two files are the same
/// episode. In testing, the same episode from different sources shared at least 0.69 and
/// different episodes at most 0.05.
pub const SAME_EPISODE: f64 = 0.3;

#[derive(Debug, Clone, PartialEq)]
pub struct Signature {
    mins: Vec<u32>,
    shingles: u32,
}

impl Signature {
    /// `None` for too little text.
    pub fn of<'a>(lines: impl IntoIterator<Item = &'a str>) -> Option<Self> {
        let chars: Vec<char> = lines
            .into_iter()
            .flat_map(|line| line.nfkc())
            .filter(|c| c.is_alphanumeric())
            .collect();
        let shingles: HashSet<u64> = chars.windows(SHINGLE_CHARS).map(fnv).collect();
        if shingles.len() < MIN_SHINGLES {
            return None;
        }
        let seeds: Vec<u64> = (1..=PERMUTATIONS as u64).map(mix).collect();
        let mins = seeds
            .iter()
            .map(|seed| shingles.iter().map(|h| (mix(h ^ seed) >> 32) as u32).min().unwrap_or(0))
            .collect();
        Some(Self { mins, shingles: shingles.len() as u32 })
    }

    /// The estimated share of the smaller text's shingles that the other text also has.
    pub fn containment(&self, other: &Signature) -> f64 {
        let agree = self.mins.iter().zip(&other.mins).filter(|(a, b)| a == b).count();
        let jaccard = agree as f64 / PERMUTATIONS as f64;
        let (a, b) = (self.shingles as f64, other.shingles as f64);
        (jaccard * (a + b) / (1.0 + jaccard) / a.min(b)).min(1.0)
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = vec![FORMAT];
        bytes.extend(self.shingles.to_le_bytes());
        bytes.extend(self.mins.iter().flat_map(|m| m.to_le_bytes()));
        bytes
    }

    pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
        let (&format, rest) = bytes.split_first()?;
        if format != FORMAT || rest.len() != 4 * (PERMUTATIONS + 1) {
            return None;
        }
        let mut words = rest.chunks_exact(4).map(|w| u32::from_le_bytes(w.try_into().unwrap()));
        let shingles = words.next()?;
        Some(Self { mins: words.collect(), shingles })
    }
}

fn fnv(chars: &[char]) -> u64 {
    chars
        .iter()
        .fold(0xcbf29ce484222325, |h, c| (h ^ u64::from(*c as u32)).wrapping_mul(0x100000001b3))
}

/// splitmix64's finalizer: spreads one hash into an independent one per seed.
fn mix(mut x: u64) -> u64 {
    x ^= x >> 30;
    x = x.wrapping_mul(0xbf58476d1ce4e5b9);
    x ^= x >> 27;
    x = x.wrapping_mul(0x94d049bb133111eb);
    x ^ (x >> 31)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Kana lines from a seeded generator, standing in for an episode's dialogue.
    fn episode(seed: u64, lines: usize) -> Vec<String> {
        let mut state = seed;
        (0..lines)
            .map(|_| {
                (0..12)
                    .map(|_| {
                        state = mix(state);
                        char::from_u32(0x3042 + (state % 80) as u32).unwrap()
                    })
                    .collect()
            })
            .collect()
    }

    #[test]
    fn another_source_of_an_episode_matches_and_another_episode_does_not() {
        let a = episode(1, 300);
        // Another source: every fifth line missing, the rest punctuated and split differently.
        let a_again: Vec<String> = a
            .iter()
            .enumerate()
            .filter(|(i, _)| i % 5 != 0)
            .map(|(_, line)| format!("「{}」\n{}…", &line[..12], &line[12..]))
            .collect();
        let b = episode(2, 300);
        let sig = |lines: &[String]| Signature::of(lines.iter().map(String::as_str)).unwrap();

        assert!(sig(&a).containment(&sig(&a_again)) > SAME_EPISODE);
        assert!(sig(&a).containment(&sig(&b)) < SAME_EPISODE);
        assert_eq!(Signature::from_bytes(&sig(&a).to_bytes()), Some(sig(&a)));
    }
}

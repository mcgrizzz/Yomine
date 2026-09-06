//! Generated JMdict/Jitendex kana preferences. No startup deserialization or heap index.
use super::frequency_manager::fold_katakana;
use crate::segmentation::word::POS;

const DATA: &[u8] = include_bytes!("../../assets/kana-preference.bin");

#[derive(Clone, Copy)]
pub struct KanaIndex<'a> {
    data: &'a [u8],
    count: usize,
}
fn u32_at(data: &[u8], offset: usize) -> Option<usize> {
    Some(u32::from_le_bytes(data.get(offset..offset.checked_add(4)?)?.try_into().ok()?) as usize)
}
impl<'a> KanaIndex<'a> {
    pub fn from_bytes(data: &'a [u8]) -> Option<Self> {
        if data.get(..8)? != b"KANAIDX2" {
            return None;
        }
        let count = u32_at(data, 8)?;
        data.get(..12usize.checked_add(count.checked_mul(16)?)?)?;
        Some(Self { data, count })
    }
    pub fn len(&self) -> usize {
        self.count
    }
    pub fn is_empty(&self) -> bool {
        self.count == 0
    }
    pub fn record(&self, index: usize) -> Option<(&'a [u8], &'a [u8])> {
        if index >= self.count {
            return None;
        }
        let offset = 12 + index * 16;
        let key = u32_at(self.data, offset)?;
        let key_len = u32_at(self.data, offset + 4)?;
        let value = u32_at(self.data, offset + 8)?;
        let value_len = u32_at(self.data, offset + 12)?;
        Some((
            self.data.get(key..key.checked_add(key_len)?)?,
            self.data.get(value..value.checked_add(value_len)?)?,
        ))
    }
    pub fn lookup(&self, key: &[u8]) -> Option<&'a [u8]> {
        let (mut low, mut high) = (0, self.count);
        while low < high {
            let mid = low + (high - low) / 2;
            let (candidate, value) = self.record(mid)?;
            match candidate.cmp(key) {
                std::cmp::Ordering::Less => low = mid + 1,
                std::cmp::Ordering::Greater => high = mid,
                std::cmp::Ordering::Equal => return Some(value),
            }
        }
        None
    }
}
pub fn bundled() -> KanaIndex<'static> {
    KanaIndex::from_bytes(DATA).expect("validated bundled kana index")
}

pub struct Preference<'a> {
    value: &'a [u8],
    pos: u32,
}
impl Preference<'_> {
    pub fn matches(&self, spelling: &str) -> bool {
        let spelling = fold_katakana(spelling);
        let Some(count) = self.value.get(4..6).map(|s| u16::from_le_bytes([s[0], s[1]])) else {
            return false;
        };
        let mut offset = 6;
        for _ in 0..count {
            let Some(mask) = u32_at(self.value, offset) else {
                return false;
            };
            let Some(len) = self
                .value
                .get(offset + 4..offset + 6)
                .map(|s| u16::from_le_bytes([s[0], s[1]]) as usize)
            else {
                return false;
            };
            offset += 6;
            let Some(form) = self.value.get(offset..offset + len) else {
                return false;
            };
            if mask as u32 & self.pos != 0 && form == spelling.as_bytes() {
                return true;
            }
            offset += len;
        }
        false
    }
}
/// Offline preferred identity for this reading and POS, independent of the user's cards.
pub fn preference(reading: &str, pos: &POS) -> Option<Preference<'static>> {
    let pos = match pos {
        POS::Noun => 1,
        POS::Verb | POS::SuruVerb => 2,
        POS::Adjective => 4,
        POS::AdjectivalNoun => 8,
        POS::Adverb => 16,
        _ => return None,
    };
    let key = format!("{}\t{}", fold_katakana(reading), pos);
    Some(Preference { value: bundled().lookup(key.as_bytes())?, pos })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn complete_asset_is_sorted_and_searchable() {
        let index = bundled();
        assert!(index.len() > 10_000);
        let mut previous = &[][..];
        for i in 0..index.len() {
            let (key, value) = index.record(i).unwrap();
            assert!(key > previous);
            assert_eq!(index.lookup(key), Some(value));
            assert!(std::str::from_utf8(key).is_ok());
            previous = key;
        }
        assert!(index.lookup(b"absent").is_none());
        assert!(KanaIndex::from_bytes(b"KANAIDX2").is_none());
        assert!(KanaIndex::from_bytes(b"KANAIDX2\xff\xff\xff\xff").is_none());
    }
    #[test]
    fn preferences_require_reading_and_pos_and_keep_rivals_separate() {
        for (reading, spelling, rival, pos) in [
            ("コト", "事", "琴", POS::Noun),
            ("できる", "出来る", "出切る", POS::Verb),
            ("いく", "行く", "逝く", POS::Verb),
            ("みせる", "見せる", "診せる", POS::Verb),
            ("なし", "無し", "梨", POS::Noun),
            ("まさに", "正に", "将に", POS::Adverb),
        ] {
            let selected = preference(reading, &pos).unwrap();
            assert!(selected.matches(spelling));
            assert!(!selected.matches(rival));
        }
        assert!(preference("はし", &POS::Noun).is_none());
        assert!(preference("こと", &POS::Verb).is_none());
        assert!(preference("みせる", &POS::Pronoun).is_none());
        assert!(preference("", &POS::Noun).is_none());
    }
}

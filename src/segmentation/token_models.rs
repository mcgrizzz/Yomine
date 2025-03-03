//Using https://github.com/jannisbecker/ve-rs/blob/main/src/lib.rs as an example for ipadic and implementing for unidic 
//Unidic POS: https://gist.github.com/masayu-a/e3eee0637c07d4019ec9

use super::unidic_tags::UnidicTag;

pub struct VibratoToken {
    pub surface: String,
    pub feature: String,
}

impl From<vibrato::token::Token<'_, '_>> for VibratoToken {
    fn from(value: vibrato::token::Token) -> Self {
        Self {
            surface: value.surface().into(),
            feature: value.feature().into(),
        }
    }
}

#[derive(serde::Deserialize)]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct RawToken {
    pub pos1: String,        // Column 1: Part of speech 1
    pub pos2: String,        // Column 2: Part of speech 2
    pub pos3: String,        // Column 3: Part of speech 3
    pub pos4: String,        // Column 4: Part of speech 4
    pub c_type: String,      // Column 5: Conjugation type
    pub c_form: String,      // Column 6: Conjugation form
    pub l_form: String,      // Column 7: Lexeme form
    pub lemma: String,       // Column 8: Lemma
    pub orth: String,        // Column 9: Orthography
    pub pron: String,        // Column 10: Pronunciation
    pub orth_base: String,   // Column 11: Orthography base
    pub pron_base: String,   // Column 12: Pronunciation base
    pub goshu: String,       // Column 13: Word origin
    pub i_type: String,      // Column 14: Initial type
    pub i_form: String,      // Column 15: Initial form
    pub f_type: String,      // Column 16: Final type
    pub f_form: String,      // Column 17: Final form
    pub i_con_type: String,  // Column 18: Initial conjugation type
    pub f_con_type: String,  // Column 19: Final conjugation type
    pub _type: String,       // Column 20: Type
    pub kana: String,        // Column 21: Kana
    pub kana_base: String,   // Column 22: Kana base
    pub form: String,        // Column 23: Form
    pub form_base: String,   // Column 24: Form base
    pub a_type: String,      // Column 25: Accent type
    pub a_con_type: String,  // Column 26: Accent connection type
    pub a_mod_type: String,  // Column 27: Accent modification type
    pub lid: String,         // Column 28: Lexicon ID
    pub lemma_id: String,    // Column 29: Lemma ID
}


#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct UnidicToken {
    pub surface: String,
    pub pos1: UnidicTag,
    pub pos2: UnidicTag,    
    pub pos3: UnidicTag,    
    pub pos4: UnidicTag,    
    pub conjugation_type: UnidicTag, 
    pub conjugation_form: UnidicTag, 

    pub surface_hatsuon: String, 
    pub lemma_form: String,         
    pub lemma_hatsuon: String,       
}

impl From<(String, RawToken)> for UnidicToken {
    fn from(item: (String, RawToken)) -> Self {
        let (surface, raw) = item;
        UnidicToken {
            surface,
            pos1: raw.pos1.as_str().into(),
            pos2: raw.pos2.as_str().into(),
            pos3: raw.pos3.as_str().into(),
            pos4: raw.pos4.as_str().into(),
            conjugation_type: raw.c_type.as_str().into(),
            conjugation_form: raw.c_form.as_str().into(),
            surface_hatsuon: raw.pron,
            lemma_form: raw.orth_base,
            lemma_hatsuon: raw.pron_base,
        }
    }
}
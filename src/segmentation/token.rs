//Using https://github.com/jannisbecker/ve-rs/blob/main/src/lib.rs as an example for ipadic and implementing for unidic 
//Unidic POS: https://gist.github.com/masayu-a/e3eee0637c07d4019ec9

use crate::core::YomineError;

struct VibratoToken {
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
struct TokenKnown {
    pos1: String,
    pos2: String,
    pos3: String,
    pos4: String,
    c_type: String,
    c_form: String,
    l_form: String,
    lemma: String,
    orth: String,
    orth_base: String,
    pron_base: String,
    goshu: String,
    i_type: String,
    i_form: String,
    f_type: String,
    f_form: String,
    i_con_type: String,
    f_con_type: String,
    _type: String,
    kana: String,
    kana_base: String,
    form: String,
    form_base: String,
    a_type: String,
    a_con_type: String,
    a_mod_type: String
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct UnidicToken {
    surface: String,
    pos: UnidicTag,     // [1]
    pos1: UnidicTag,    // [2]
    pos2: UnidicTag,    // [3]
    pos3: UnidicTag,    // [4]
    inflection_type: UnidicTag, //c_type [5]
    inflection_form: UnidicTag, //c_form [6] //only up to here are present if word is unknown

    lemma_form: String,         //lemma [8]
    surface_form: String,       //orth [9]
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum UnidicTag {
    // Main POS categories
    Daimeshi, // Pronoun (代名詞)
    Fukushi, // Adverb (副詞)
    Jodoushi, // Auxiliary verb (助動詞)
    Doushi, // Verb (動詞)
    Joshi, // Particle (助詞)
    Meishi, // Noun (名詞)
    Keiyoushi, // Adjective (形容詞)
    Keijoushi, // Adjectival noun (形状詞)
    Setsuzokushi, // Conjunction (接続詞)
    Kandoushi, // Interjection (感動詞)
    Rentaishi, // Adnominal (連体詞) (あの, この, etc)
    Kigou, // Symbol (記号)

    // Noun types
    Koyuumeishi, // Proper noun (固有名詞)
    Futsuumeishi, // Common noun (普通名詞)
    Suushi, // Numeral (数詞)

    // Proper nouns (subtypes)
    Jinmei, // Person's name (人名)
    Mei, // First name (名)
    Sei, // Family name (姓)
    Chimei, // Place name (地名)
    Kuni, // Country (国)

    // Verb-related categories
    Jodoushigokan, // Auxiliary verb stem (助動詞語幹)
    Sahenkanou, // Noun that can take "suru" (サ変可能)
    Sahenkeijoushikanou, // Adjectival noun that can take "suru" (サ変形状詞可能)

    // Bound/Convertible Noun Types
    Ippan, // General (一般)
    Hijiritsukanou, // Bound word (非自立可能)
    Keijoushikanou, // Adjectival noun (形状詞可能)
    Josuushikanou, // Noun that can function as a counter (助数詞可能)
    Fukushikanou, // Noun that can function as an adverb (副詞可能)

    // Particles (助詞)
    Kakarijoshi, // Binding particle (係助詞)
    Fukujoshi, // Adverbial particle (副助詞)
    Setsuzokujoshi, // Conjunctive particle (接続助詞)
    Kakujoshi, // Case-marking particle (格助詞)
    Juntaijoshi, // Nominalizing particle (準体助詞)
    Shuujoshi, // Sentence-ending particle (終助詞)

    // Other POS categories
    Settouji, // Prefix (接頭辞)
    Setsubiji, // Suffix (接尾辞)
    Meishiteki, // Nominal suffix (名詞的)
    Doushiteki, // Verbal suffix (動詞的)
    Keiyoushiteki, // Adjectival suffix (形容詞的)
    Keijoushiteki, // Adjectival noun suffix (形状詞的)
    Josuushi, //Counter Word

    // Symbols & Special Characters
    Kutouten, // Punctuation (句点, 読点)
    Kuten, //Period (句点)
    Kakkoaki, // Opening bracket (括弧開)
    Kakkotoji, // Closing bracket (括弧閉)
    Touten, //Comma (読点)
    Aa, // ASCII Art (ＡＡ)
    Kaomoji, // Emoticon (顔文字)

    // Miscellaneous
    Kuuhaku, // Whitespace (空白)
    Webgodatsu, // Web-based errors (web誤脱)
    Hougen, // Dialect (方言)
    Firaa, // Filler (フィラー)
    Gidai, // Hesitation (言いよどみ)
    Michigo, // Unknown word (未知語)
    Shinkimichigo, // Newly discovered unknown word (新規未知語)
    Katakanabun, // Sentence in all katakana (カタカナ文)
    Roumajibun, // Sentence in Roman letters (ローマ字文)
    Kanbun, // Classical Chinese (漢文)

    //Inflection types 5.2 活用型 *That we care about*
    //Auxillary verbs
    JodoushiDa, //da (助動詞-ダ)
    JodoushiTa, //ta (助動詞-タ)
    JodoushiNu, //nu (助動詞-ヌ)
    JodoushiMai, //mai (助動詞-マイ)
    JodoushiNai, //nai 助動詞-ナイ
    JodoushiTai, //tai 助動詞-タイ
    JodoushiDesu, //desu 助動詞-デス
    JodoushiRashii, //rashii 助動詞-ラシイ
    JodoushiMasu, //masu 助動詞-マス
    JodoushiReru, //reru 助動詞-レル

    //Classical Auxillaries.. Not sure we need these
    BungojodoushiNari, //文語助動詞-ナリ-断定
    BungojodoushiBeshi, //文語助動詞-ベシ

    //Others
    Sagyouhenkaku, //サ行変格
    Kagyouhenkaku, //カ行変格
    //Inflection forms 5.3 活用形


    //Non-unidic types
    Unset, // *
    Unknown, //Different than Michigo, if for some reason we get an unknown POS output we will match to this
}

impl From<&str> for UnidicTag {
    fn from(value: &str) -> Self {
        match value {
            "代名詞"        => Self::Daimeshi,
            "副詞"          => Self::Fukushi,
            "助動詞"        => Self::Jodoushi,
            "動詞"          => Self::Doushi,
            "助詞"          => Self::Joshi,
            "名詞"          => Self::Meishi,
            "形容詞"        => Self::Keiyoushi,
            "形状詞"        => Self::Keijoushi,
            "接続詞"        => Self::Setsuzokushi,
            "感動詞"        => Self::Kandoushi,
            "連体詞"        => Self::Rentaishi,
            "記号"          => Self::Kigou,

            "固有名詞"      => Self::Koyuumeishi,
            "普通名詞"      => Self::Futsuumeishi,
            "数詞"          => Self::Suushi,

            "人名"          => Self::Jinmei,
            "名"            => Self::Mei,
            "姓"            => Self::Sei,
            "地名"          => Self::Chimei,
            "国"            => Self::Kuni,

            "助動詞語幹"    => Self::Jodoushigokan,
            "サ変可能"      => Self::Sahenkanou,
            "サ変形状詞可能" => Self::Sahenkeijoushikanou,

            "一般"          => Self::Ippan,
            "非自立可能"    => Self::Hijiritsukanou,
            "形状詞可能"    => Self::Keijoushikanou,
            "助数詞可能"    => Self::Josuushikanou,
            "副詞可能"      => Self::Fukushikanou,

            "係助詞"        => Self::Kakarijoshi,
            "副助詞"        => Self::Fukujoshi,
            "接続助詞"      => Self::Setsuzokujoshi,
            "格助詞"        => Self::Kakujoshi,      
            "準体助詞"      => Self::Juntaijoshi,    
            "終助詞"        => Self::Shuujoshi,

            "補助記号"      => Self::Kutouten,       
            "句点"          => Self::Kuten,         
            "括弧開"        => Self::Kakkoaki,       
            "括弧閉"        => Self::Kakkotoji,      
            "読点"          => Self::Touten,
            "ＡＡ"          => Self::Aa,
            "顔文字"        => Self::Kaomoji,

            "接頭辞"        => Self::Settouji,        
            "接尾辞"        => Self::Setsubiji,      
            "名詞的"        => Self::Meishiteki,  
            "動詞的"        => Self::Doushiteki,  
            "形容詞的"      => Self::Keiyoushiteki, 
            "形状詞的"      => Self::Keijoushiteki,
            "助数詞"        => Self::Josuushi, 

            "空白"          => Self::Kuuhaku,        
            "web誤脱"       => Self::Webgodatsu,     
            "方言"          => Self::Hougen,
            "フィラー"      => Self::Firaa,         
            "言いよどみ"    => Self::Gidai,         
            "未知語"        => Self::Michigo,         
            "新規未知語"    => Self::Shinkimichigo, 
            "カタカナ文"    => Self::Katakanabun,    
            "ローマ字文"    => Self::Roumajibun,     
            "漢文"          => Self::Kanbun,

            "助動詞-ダ"     => Self::JodoushiDa,
            "助動詞-タ"     => Self::JodoushiTa,
            "助動詞-ヌ"     => Self::JodoushiNu,
            "助動詞-マイ"   => Self::JodoushiMai,
            "助動詞-ナイ"   => Self::JodoushiNai,
            "助動詞-タイ"   => Self::JodoushiTai,
            "助動詞-デス"   => Self::JodoushiDesu,
            "助動詞-ラシイ" => Self::JodoushiRashii,
            "助動詞-マス"   => Self::JodoushiMasu,
            "助動詞-レル"   => Self::JodoushiReru,

            "文語助動詞-ナリ-断定"  => Self::BungojodoushiNari,
            "文語助動詞-ベシ"       => Self::BungojodoushiBeshi,

            "*"            => Self::Unset,         

            _ => {
                UnidicTag::Unknown
            },
        }
    }
}

enum POS {
    Noun,
    ProperNoun,
    Pronoun,
    Adjective,
    Adverb,
    Determiner,
    Preposition, 
    Postposition, //Like auxillary verbs
    Verb,
    Suffix,
    Prefix,
    Conjunction,
    Interjection, 
    Number,
    Symbol,
    Other,
    Unknown,
}

struct Word {
    pub surface_form: String, 
    pub surface_hatsuon: String, //hatsuon is easier to type than pronunciation...
    pub lemma_form: String,
    pub lemma_hatsuon: String,
    pub part_of_speech: POS,
    pub tokens: Vec<UnidicToken>,
}

fn parse_into_words(tokens: Vec<UnidicToken>) -> Result<Vec<Word>, YomineError> {
    let mut words: Vec<Word> = Vec::new();
    let mut iter = tokens.iter().peekable();
    let mut previous: Option<UnidicToken> = None;

    while let Some(token) = iter.next() {
        let mut pos: Option<POS> = None;

        //Forward looking modifications
        let mut eat_next = false; //Eat the next token, we will incorporate into the current token (surface_hatsoun and surface_form are appended)
        let mut eat_next_lemma = false; //Append the lemma from the next token to this one (also the lemma_hatsuon to this lemma_hatsuon)

        //backwards looking modifications
        let mut attach_prev = false; //Attach surface_hatsuon and surface_form to previous token
        let mut attach_prev_lemma = false; //Attach lemma and lemma_hatsuon to previous token
        let mut update_prev_pos = false; //Change last token POS 

        match token.pos {
            UnidicTag::Meishi => { //Noun
                pos = Some(POS::Noun);
            },
            UnidicTag::Settouji => { //Prefix
                pos = Some(POS::Prefix);
            },
            UnidicTag::Jodoushi => { //Auxilliary verb
                pos = Some(POS::Postposition);
            },
            UnidicTag::Doushi => { //verb
                pos = Some(POS::Verb);
            },
            UnidicTag::Keiyoushi => { //Adjective
                pos = Some(POS::Adjective);
            },
            UnidicTag::Joshi => { //Particle
                pos = Some(POS::Postposition);
            },
            UnidicTag::Rentaishi => { //Adnominal
                pos = Some(POS::Determiner);
            },
            UnidicTag::Setsuzokushi => { //Conjunction
                pos = Some(POS::Conjunction);
            },
            UnidicTag::Fukushi => { //Adverb
                pos = Some(POS::Adverb);
            },
            UnidicTag::Kigou => { //Symbol
                pos = Some(POS::Symbol);
            },
            UnidicTag::Firaa | UnidicTag::Kandoushi => { //Filler and Interjections
                pos = Some(POS::Interjection);
            },
            UnidicTag::Michigo => { //Unknown 
                pos = Some(POS::Other);
            },
            _ => (),
        }

        if pos.is_none() {
            return Err(YomineError::Custom(format!("Part of speech couldn't be recognized for token {}", token.surface)));
        }

        let pos = pos.unwrap();

        if attach_prev && words.len() > 0 {
            let last = words.last_mut().unwrap();

            let token = token.clone();

            last.surface_form.push_str(&token.surface);
            last.surface_hatsuon.push_str(&token.);

        }

    }

    todo!();
}

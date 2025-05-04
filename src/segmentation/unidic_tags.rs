#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum UnidicTag {
    // Main POS categories
    Daimeshi, // Pronoun (代名詞) //pos1
    Fukushi, // Adverb (副詞) //pos1
    Jodoushi, // Auxiliary verb (助動詞) //pos1
    Doushi, // Verb (動詞) //pos1
    Joshi, // Particle (助詞) //pos1
    Meishi, // Noun (名詞) //pos1
    Keiyoushi, // Adjective (形容詞) //pos1
    Keijoushi, // Adjectival noun (形状詞) //pos1
    Setsuzokushi, // Conjunction (接続詞)
    Kandoushi, // Interjection (感動詞) //pos1
    Rentaishi, // Adnominal (連体詞) (あの, この, etc) //pos1
    Kigou, // Symbol (記号) //pos1
    Settouji, // Prefix (接頭辞) //pos1
    Setsubiji, // Suffix (接尾辞) //pos1
    Meishiteki, // Nominal suffix (名詞的) //pos1
    Hojokigou, // Supplementary symbols (補助記号) pos1
    Kuuhaku, // Whitespace (空白) //pos1

    // Noun types
    Koyuumeishi, // Proper noun (固有名詞) //pos2
    Futsuumeishi, // Common noun (普通名詞) //pos2
    Suushi, // Numeral (数詞) //pos2

    // Proper nouns (subtypes)
    Jinmei, // Person's name (人名) //pos3
    Mei, // First name (名) //pos4
    Sei, // Family name (姓) //pos4
    Chimei, // Place name (地名) //pos3
    Kuni, // Country (国) //pos4

    // Verb-related categories
    Jodoushigokan, // Auxiliary verb stem (助動詞語幹) //pos2
    Sahenkanou, // Noun that can take "suru" (サ変可能) //pos3
    Sahenkeijoushikanou, // Adjectival noun that can take "suru" (サ変形状詞可能) //pos3

    // Bound/Convertible Noun Types
    Ippan, // General (一般) //pos2, pos3, pos4
    Hijiritsukanou, // Bound word (非自立可能) //pos2
    Keijoushikanou, // Adjectival noun (形状詞可能) //pos3
    Josuushikanou, // Nctoun that can funion as a counter (助数詞可能) //pos3
    Fukushikanou, // Noun that can function as an adverb (副詞可能) //pos3

    // Particles (助詞)
    Kakarijoshi, // Binding particle (係助詞) //pos2
    Fukujoshi, // Adverbial particle (副助詞) //pos2
    Setsuzokujoshi, // Conjunctive particle (接続助詞) //pos2
    Kakujoshi, // Case-marking particle (格助詞) //pos2
    Juntaijoshi, // Nominalizing particle (準体助詞) //pos2
    Shuujoshi, // Sentence-ending particle (終助詞) //pos2

    // Other POS categories
    Doushiteki, // Verbal suffix (動詞的) //can't find yet
    Keiyoushiteki, // Adjectival suffix (形容詞的) //pos2
    Keijoushiteki, // Adjectival noun suffix (形状詞的) //can't find yet
    Josuushi, //Counter Word (助数) //col20 (type)

    // Symbols & Special Characters
    Kuten, //Period (句点) //pos2
    Kakkoaki, // Opening bracket (括弧開) //pos2
    Kakkotoji, // Closing bracket (括弧閉) //pos2
    Touten, //Comma (読点) //pos2
    Aa, // ASCII Art (ＡＡ)
    Kaomoji, // Emoticon (顔文字)

    // Miscellaneous
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
    JodoushiDa, //da,na (助動詞-ダ) //conjugation_type
    JodoushiTa, //ta (助動詞-タ) //conjugation_type
    JodoushiNu, //nu (助動詞-ヌ)
    JodoushiMai, //mai (助動詞-マイ) //
    JodoushiNai, //nai 助動詞-ナイ //conjugation_type
    JodoushiTai, //tai 助動詞-タイ //conjugation_type
    JodoushiDesu, //desu 助動詞-デス //conjugation_type
    JodoushiRashii, //rashii 助動詞-ラシイ
    JodoushiMasu, //masu 助動詞-マス //conjugation_type
    JodoushiReru, //reru 助動詞-レル

    //Classical Auxillaries.. Not sure we need these
    BungojodoushiNari, //文語助動詞-ナリ-断定
    BungojodoushiBeshi, //文語助動詞-ベシ

    //Others
    Sagyouhenkaku, //サ行変格 //conjugation_type
    Kagyouhenkaku, //カ行変格 //conjugation_type
    //Inflection forms 5.3 活用形

    //Non-unidic types
    Unset, // *
    Unknown, //Different than Michigo, if for some reason we get an unknown POS output we will match to this
}

impl From<&str> for UnidicTag {
    fn from(value: &str) -> Self {
        match value {
            "代名詞" => Self::Daimeshi,
            "副詞" => Self::Fukushi,
            "助動詞" => Self::Jodoushi,
            "動詞" => Self::Doushi,
            "助詞" => Self::Joshi,
            "名詞" => Self::Meishi,
            "形容詞" => Self::Keiyoushi,
            "形状詞" => Self::Keijoushi,
            "接続詞" => Self::Setsuzokushi,
            "感動詞" => Self::Kandoushi,
            "連体詞" => Self::Rentaishi,
            "記号" => Self::Kigou,

            "固有名詞" => Self::Koyuumeishi,
            "普通名詞" => Self::Futsuumeishi,
            "数詞" => Self::Suushi,

            "人名" => Self::Jinmei,
            "名" => Self::Mei,
            "姓" => Self::Sei,
            "地名" => Self::Chimei,
            "国" => Self::Kuni,

            "助動詞語幹" => Self::Jodoushigokan,
            "サ変可能" => Self::Sahenkanou,
            "サ変形状詞可能" => Self::Sahenkeijoushikanou,

            "一般" => Self::Ippan,
            "非自立可能" => Self::Hijiritsukanou,
            "形状詞可能" => Self::Keijoushikanou,
            "助数詞可能" => Self::Josuushikanou,
            "副詞可能" => Self::Fukushikanou,

            "係助詞" => Self::Kakarijoshi,
            "副助詞" => Self::Fukujoshi,
            "接続助詞" => Self::Setsuzokujoshi,
            "格助詞" => Self::Kakujoshi,
            "準体助詞" => Self::Juntaijoshi,
            "終助詞" => Self::Shuujoshi,

            "補助記号" => Self::Hojokigou,
            "句点" => Self::Kuten,
            "括弧開" => Self::Kakkoaki,
            "括弧閉" => Self::Kakkotoji,
            "読点" => Self::Touten,
            "ＡＡ" => Self::Aa,
            "顔文字" => Self::Kaomoji,

            "接頭辞" => Self::Settouji,
            "接尾辞" => Self::Setsubiji,
            "名詞的" => Self::Meishiteki,
            "動詞的" => Self::Doushiteki,
            "形容詞的" => Self::Keiyoushiteki,
            "形状詞的" => Self::Keijoushiteki,
            "助数詞" => Self::Josuushi,

            "空白" => Self::Kuuhaku,
            "web誤脱" => Self::Webgodatsu,
            "方言" => Self::Hougen,
            "フィラー" => Self::Firaa,
            "言いよどみ" => Self::Gidai,
            "未知語" => Self::Michigo,
            "新規未知語" => Self::Shinkimichigo,
            "カタカナ文" => Self::Katakanabun,
            "ローマ字文" => Self::Roumajibun,
            "漢文" => Self::Kanbun,

            "助動詞-ダ" => Self::JodoushiDa,
            "助動詞-タ" => Self::JodoushiTa,
            "助動詞-ヌ" => Self::JodoushiNu,
            "助動詞-マイ" => Self::JodoushiMai,
            "助動詞-ナイ" => Self::JodoushiNai,
            "助動詞-タイ" => Self::JodoushiTai,
            "助動詞-デス" => Self::JodoushiDesu,
            "助動詞-ラシイ" => Self::JodoushiRashii,
            "助動詞-マス" => Self::JodoushiMasu,
            "助動詞-レル" => Self::JodoushiReru,

            "文語助動詞-ナリ-断定" => Self::BungojodoushiNari,
            "文語助動詞-ベシ" => Self::BungojodoushiBeshi,

            "サ行変格" => Self::Sagyouhenkaku,
            "カ行変格" => Self::Kagyouhenkaku,

            "*" => Self::Unset,

            _ => { UnidicTag::Unknown }
        }
    }
}

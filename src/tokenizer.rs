use vibrato::{tokenizer::worker::Worker, Tokenizer};
use crate::parser::{ParsedFile, Phrase, Word};
use crate::dictionary::{ensure_dictionary, load_dictionary, DictType};
use crate::YomineError;

pub fn extract_words(mut worker: Worker<'_>, parsed_file: ParsedFile<impl Phrase>) -> Vec<Word> {
    let words = Vec::<Word>::new();
    for phrase in &parsed_file.phrases {
        worker.reset_sentence(&phrase.get_phrase());
        worker.tokenize();

        println!("\ntext:\t{}", phrase.get_phrase());
        for token in worker.token_iter() {
            let details = token.feature();
            println!("token:\t{}\t{}", token.surface(), details);
        }
    }

    words
}

pub fn init_vibrato(dict_type: DictType) -> Result<Tokenizer, YomineError> {
    let dict_path = ensure_dictionary(dict_type)?;
    let dict = load_dictionary(dict_path.to_str().unwrap())?;
    let tokenizer = vibrato::Tokenizer::new(dict);
    Ok(tokenizer)
}
//! Read-only UniDic corpus export for research/lexical; no Anki or dictionary-cache writes.
use std::{
    fs::File,
    io::{
        BufRead,
        BufReader,
        BufWriter,
        Write,
    },
};

use serde_json::{
    json,
    Value,
};
use yomine::{
    dictionary::token_dictionary::DictType,
    segmentation::{
        token_models::{
            RawToken,
            VibratoToken,
        },
        tokenizer::init_vibrato,
    },
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.len() != 2 {
        return Err("usage: lexical_research INPUT.jsonl OUTPUT.jsonl".into());
    }
    let tokenizer = init_vibrato(&DictType::Unidic, None)?;
    let mut worker = tokenizer.new_worker();
    let mut output = BufWriter::new(File::options().write(true).create_new(true).open(&args[1])?);
    let mut count = 0;
    for line in BufReader::new(File::open(&args[0])?).lines() {
        let mut sentence: Value = serde_json::from_str(&line?)?;
        let text = sentence["text"].as_str().ok_or("missing sentence text")?;
        worker.reset_sentence(text);
        worker.tokenize();
        let tokens: Vec<_> = worker
            .token_iter()
            .map(|t| {
                let raw: RawToken =
                    VibratoToken { surface: t.surface().into(), features: t.feature().into() }
                        .into();
                json!({"surface":t.surface(),"lemma":raw.orth_base,"lexeme":raw.lemma,
                "reading":raw.kana_base,"pronunciation":raw.pron_base,"pos":raw.pos1,
                "pos2":raw.pos2,"verb_class":raw.c_type})
            })
            .collect();
        sentence["tokens"] = json!(tokens);
        serde_json::to_writer(&mut output, &sentence)?;
        writeln!(output)?;
        count += 1;
    }
    output.flush()?;
    eprintln!("Exported {count} sentences with installed UniDic");
    Ok(())
}

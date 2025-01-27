use std::{collections::HashMap, time::Duration};

use api::{get_deck_ids, get_field_names, get_model_ids, get_note_ids, get_notes, get_version, Note};
use tokio::{task::{self}, time::sleep};

pub mod api;

#[derive(Debug)]
pub struct Model {
    name: String,
    id: u64,
    fields: Vec<String>,
}

#[derive(Debug)]
pub struct FieldMapping {
    pub term_field: String, //Expression
    pub reading_field: String, //ExpressionReading
}

#[derive(Debug)]
pub struct Vocab {
    pub term: String,
    pub reading: String,
}


pub async fn get_models() -> Result<Vec<Model>, reqwest::Error> {
    let model_ids = get_model_ids().await?;

    let handles: Vec<_> = model_ids
        .into_iter()
        .map(|(model_name, id)| {
            task::spawn(async move {
                let fields = get_field_names(&model_name).await?;
                Ok::<Model, reqwest::Error>(Model {
                    name: model_name,
                    id,
                    fields,
                })
            })
        })
        .collect();

    let models: Vec<Model> = futures::future::join_all(handles)
        .await
        .into_iter()
        .filter_map(|result| result.ok()) // Task errors
        .filter_map(|inner_result| inner_result.ok()) // API errors
        .collect();

    Ok(models)
}


pub async fn get_total_vocab() -> Result<Vec<Vocab>, reqwest::Error> {

    //temporary manually define models
    let mut model_mapping: HashMap<String, FieldMapping> = HashMap::new();
    model_mapping.insert(
        "Lapis".to_string(), 
        FieldMapping {
            term_field: "Expression".to_string(),
            reading_field: "ExpressionReading".to_string(),
        });

        model_mapping.insert(
            "Kaishi 1.5k".to_string(), 
            FieldMapping {
                term_field: "Word".to_string(),
                reading_field: "Word Reading".to_string(),
            });

    let decks = get_deck_ids().await?;
    let deck_names: Vec<String> = decks
        .into_iter()
        .map(|deck| {
            let deck_name = deck.name;
            format!("deck:\"{deck_name}\"")
        })
        .collect();

    let deck_query = deck_names.join(" OR ");
    println!("{deck_query}");

    let note_ids = get_note_ids(&deck_query).await?;
    let notes = get_notes(note_ids).await?;
    
    let vocab: Vec<Vocab> = notes
        .into_iter()
        .filter_map(|note| {
            if let Some(field_mapping) = model_mapping.get(&note.model_name) {

                let term = note.fields.get(&field_mapping.term_field).map(|f| f.value.clone());
                let reading = note.fields.get(&field_mapping.reading_field).map(|f| f.value.clone());

                if let (Some(term), Some(reading)) = (term, reading) {
                    return Some(Vocab { term, reading });
                }
            }
            None
        })
        .collect();

    Ok(vocab)
}

pub async fn wait_awake(wait_time: u64, max_attempts: u32) -> Result<bool, reqwest::Error> {
    for attempt in 1..=max_attempts {
        match get_version().await {
            Ok(version) => {
                println!("AnkiConnect is online. Version: {}", version);
                return Ok(true);
            }
            Err(err) => {
                println!(
                    "AnkiConnect attempt {} of {} failed. Retrying in {} seconds... Error: {}",
                    attempt, max_attempts, wait_time, err
                );
                if attempt < max_attempts {
                    sleep(Duration::from_secs(wait_time)).await;
                }
            }
        }
    }

    println!("AnkiConnect did not respond after {} attempts.", max_attempts);
    Ok(false)
}
//This segmentation should be done based on frequency dictionaries and de-inflection like yomitan. Which should give us good overlap with yomitan for mining

use std::{collections::{HashMap, HashSet}, u32};

use jp_deinflector::deinflect;

use crate::{core::utils::harmonic_frequency, frequency_dict::{self, FrequencyManager}};

#[derive(Clone)]
pub struct Token {
    pub surface_form: String, //How it is found in the sentence
    frequency: Option<u32>, //Frequency for the surface form
    deinflected_forms: Vec<(String, u32)>, //(deinflected_form, frequency)
}

impl Token {
    pub fn new(surface_form: String) -> Self {
        Token {
            surface_form,
            frequency: None,
            deinflected_forms: Vec::new(),
        }
    }
}

impl FrequencyManager {
    /// Cached frequency lookup
    fn get_cached_harmonic(&self, text: &str, cache: &mut HashMap<String, u32>) -> u32 {
        if let Some(&freq) = cache.get(text) {
            return freq;
        }
        
        let freqs: Vec<u32> = self.get_exact_frequency(text)
            .iter()
            .map(|freq| freq.value())
            .collect();

        let harmonic_freq = harmonic_frequency(&freqs).unwrap_or(u32::MAX);
        cache.insert(text.to_string(), harmonic_freq);
        harmonic_freq
    }
}

pub struct SegmentationCache {
    frequency_cache: HashMap<String, u32>, // Cached harmonic frequencies
    deinflection_cache: HashMap<String, (Vec<(String, u32)>, f32)>, // Cached deinflected forms
    node_cache: HashMap<String, Vec<SegmentationNode>>, // Cached segmentation nodes
}

impl SegmentationCache {
    pub fn new() -> Self {
        Self {
            frequency_cache: HashMap::new(),
            deinflection_cache: HashMap::new(),
            node_cache: HashMap::new(),
        }
    }
}

#[derive(Clone)]
pub struct SegmentationNode {
    pub token: Token,
    score: f32, //Calculated based on children at the end
    children: Vec<SegmentationNode>, 
}

impl SegmentationNode {

    pub fn new(token: Token, score: f32) -> Self {
        SegmentationNode {
            token,
            score,
            children: Vec::new(),
        }
    }

    fn get_seg_path_span(&self) -> (usize, usize) {
        if self.children.is_empty() {
            return (1, 1);
        }

        let child_spans: Vec<(usize, usize)> = self.children
            .iter()
            .map(|child| child.get_seg_path_span())
            .collect();

        let min_len = child_spans.iter().map(|(min, _)| min).min().unwrap() + 1;
        let max_len = child_spans.iter().map(|(_, max)| max).max().unwrap() + 1;

        (min_len, max_len)
    }

    fn get_best_segmentation(&self, visited: &mut HashSet<String>) -> Vec<&SegmentationNode> {
        let mut best_path = vec![self];

        let mut current_node = self;
        while let Some(best_child) = current_node
            .children
            .iter()
            .filter(|c| !visited.contains(&c.token.surface_form)) // Ignore already visited nodes
            .max_by(|a, b| a.score.partial_cmp(&b.score).unwrap())
        {
            visited.insert(best_child.token.surface_form.clone());
            best_path.push(best_child);
            current_node = best_child;
        }

        best_path
    }

    pub fn get_n_best_segments(&self, n: usize) -> Vec<Vec<&SegmentationNode>> {
        let mut best_segmentations = Vec::new();
        let mut visited = HashSet::new();

        for _ in 0..n {
            let best_segmentation = self.get_best_segmentation(&mut visited);
            if best_segmentation.is_empty() {
                break;
            }
            best_segmentations.push(best_segmentation);
        }

        best_segmentations
    }
    
    pub fn print_node(&self, prefix: &str, is_last: bool) {
        let fork = if is_last { "└── " } else { "├── " };
        let child_prefix = if is_last { "   " } else { "│  " };

        println!(
            "{}{}{} (Score: {:.2}) [{}]",
            prefix,
            fork,
            self.token.surface_form,
            self.score,
            self.token.deinflected_forms.iter().map(|t| t.0.clone()).collect::<Vec<String>>().join(", ")
        );

        let child_count = self.children.len();
        for (i, child) in self.children.iter().enumerate() {
            child.print_node(&format!("{}{}", prefix, child_prefix), i == child_count - 1);
        }
    }
}

//Returns a list of valid deinflections with their frequency and the average harmonic frequency
fn get_valid_deinflections(text: &str, frequency_manager: &FrequencyManager, cache: &mut SegmentationCache) -> (Vec<(String, u32)>, f32) {
    if let Some(deinflections) = cache.deinflection_cache.get(text) {
        return deinflections.clone();
    }

    let mut valid = deinflect(text);
    let mut sum_harmonic = 0;
    let mut count = 0;

    let valid_defl: Vec<(String, u32)> = valid.into_iter().filter_map(|txt| {
        let freqencies = frequency_manager.get_exact_frequency(&txt);
        if freqencies.len() > 0 {
            let freqs = freqencies.iter().map(|freq| freq.value()).collect();
            let local_harmonic = harmonic_frequency(&freqs).unwrap(); //This should always be safe to unwrap
            sum_harmonic += local_harmonic;
            count += 1;
            return Some((txt, local_harmonic));
        } 

        return None;
        
    }).collect();

    let harmonic_avg = sum_harmonic as f32 / count as f32;

    cache.deinflection_cache.insert(text.to_string(), (valid_defl.clone(), harmonic_avg));

    (valid_defl, harmonic_avg)
}


//Return segments up the longest valid one. 人死にの価値... ->  [人, 人死, 人死に] ignoring の and later since 人死にの is not a valid segmentation
fn get_nodes(text: &str, frequency_manager: &FrequencyManager, cache: &mut SegmentationCache) -> Vec<SegmentationNode> {

    if let Some(cached_nodes) = cache.node_cache.get(text) {
        return cached_nodes.clone();
    }

    let mut nodes: Vec<SegmentationNode> = Vec::new();
    
    let chars: Vec<char> = text.chars().collect();
    //
    for end in 1..chars.len() {
        let substring = String::from_iter(&chars[..end]);
        let mut freqs:Vec<u32> = Vec::new();

        let mut token = Token::new(substring.clone());
       
        let freq = frequency_manager.get_cached_harmonic(&substring, &mut cache.frequency_cache);
        if freq != u32::MAX {
            token.frequency = Some(freq);
            freqs.push(freq);
        }

        //Deinflect
        let (mut de_infl, harmonic_mean) = get_valid_deinflections(&substring, frequency_manager, cache);
        if de_infl.len() > 0 {
            freqs.push(harmonic_mean as u32);
            token.deinflected_forms.append(&mut de_infl);
        }

        //We need at least the reading or deinflected form to be in a dictionary.
        if freqs.len() > 0 {
            nodes.push(SegmentationNode::new(token, 0.0));
        }

    }
    
    cache.node_cache.insert(text.to_string(), nodes.clone());
    nodes
}

//Recursively build the segmentation tree
fn build_segmentation(search_txt: &str, node: &mut SegmentationNode, frequency_manager: &FrequencyManager, cache: &mut SegmentationCache) -> SegmentationNode {    
    let potential_nodes = get_nodes(&search_txt, frequency_manager, cache);

    for mut p_node in potential_nodes {
        let segmented_child = build_segmentation(&search_txt[p_node.token.surface_form.len()..], &mut p_node, frequency_manager, cache);
        node.children.push(segmented_child);
    }

    let (min_segs, max_segs) = node.get_seg_path_span();
    let avg_child_score = node.children.iter().map(|n| n.score).sum::<f32>() / node.children.len().max(1) as f32;

    let mut freqs: Vec<u32> = Vec::new();

    if let Some(frequency) = node.token.frequency {
        freqs.push(frequency);
    }

    if node.token.deinflected_forms.len() > 0 {
        freqs.extend(node.token.deinflected_forms.iter().map(|t| t.1));
    }

    let token_frequency = harmonic_frequency(&freqs).unwrap_or(u32::MAX);

    let alpha = 1.1; //Weight longer segments
    let beta = 0.9; //Weight frequency (lower = more weight)
    let gamma = 1.0; // Weight children_scores
    let delta = 0.75; // Segmentation penalty

    let segment_penalty = delta / (max_segs as f32 + 1.0);

    node.score =  (alpha*node.token.surface_form.chars().count() as f32)
        + (beta / ((token_frequency) as f32 + 1.0))
        + gamma*avg_child_score
        - segment_penalty;
    
    
    node.clone()
}

pub fn segment(text: &str, frequency_manager: &FrequencyManager, cache: &mut SegmentationCache) -> SegmentationNode {
    let mut root = SegmentationNode::new(Token::new("".to_string()), 0.0);
    build_segmentation(text, &mut root, frequency_manager, cache)
}
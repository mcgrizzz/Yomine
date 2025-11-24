use std::{
    collections::HashMap,
    path::PathBuf,
};

pub const BYTES_TO_MB: f64 = 1_048_576.0;
pub const TRIM_PERCENTAGE: f64 = 0.1;

pub struct CorpusBalancer {
    files: Vec<PathBuf>,
}

impl CorpusBalancer {
    pub fn new(files: Vec<PathBuf>) -> Self {
        Self { files }
    }

    pub fn balance(&self) -> Vec<PathBuf> {
        let source_files = self.group_files_by_source();

        if source_files.len() <= 1 {
            println!("Balance corpus: Only one source detected, no balancing applied");
            return self.files.clone();
        }

        let mut source_sizes = self.calculate_source_sizes(&source_files);
        let trimmed_mean_bytes = self.calculate_trimmed_mean(&source_sizes);

        self.log_balance_header(&source_sizes, trimmed_mean_bytes);

        source_sizes.sort_by(|a, b| b.1.cmp(&a.1));

        let balanced_files =
            self.perform_balancing(&source_files, &source_sizes, trimmed_mean_bytes);

        self.log_balance_summary(&balanced_files, self.files.len());

        balanced_files
    }

    fn group_files_by_source(&self) -> HashMap<String, Vec<(PathBuf, u64)>> {
        use std::fs;

        let mut source_files: HashMap<String, Vec<(PathBuf, u64)>> = HashMap::new();

        for file_path in &self.files {
            if let Some(parent) = file_path.parent() {
                let source_key =
                    parent.file_name().and_then(|n| n.to_str()).unwrap_or("unknown").to_string();

                if let Ok(metadata) = fs::metadata(file_path) {
                    source_files
                        .entry(source_key)
                        .or_insert_with(Vec::new)
                        .push((file_path.clone(), metadata.len()));
                }
            }
        }

        source_files
    }

    fn calculate_source_sizes(
        &self,
        source_files: &HashMap<String, Vec<(PathBuf, u64)>>,
    ) -> Vec<(String, u64, usize)> {
        source_files
            .iter()
            .map(|(name, files)| {
                let total_size: u64 = files.iter().map(|(_, size)| size).sum();
                let file_count = files.len();
                (name.clone(), total_size, file_count)
            })
            .collect()
    }

    fn calculate_trimmed_mean(&self, source_sizes: &[(String, u64, usize)]) -> u64 {
        let num_sources = source_sizes.len();
        let trim_count = ((num_sources as f64 * TRIM_PERCENTAGE).floor() as usize / 2) * 2;
        let trim_each_side = trim_count / 2;

        let mut sizes_only: Vec<u64> = source_sizes.iter().map(|(_, size, _)| *size).collect();
        sizes_only.sort_unstable();

        if num_sources > trim_count && trim_each_side > 0 {
            let trimmed = &sizes_only[trim_each_side..sizes_only.len() - trim_each_side];
            let sum: u64 = trimmed.iter().sum();
            sum / trimmed.len() as u64
        } else {
            let sum: u64 = sizes_only.iter().sum();
            sum / sizes_only.len() as u64
        }
    }

    fn log_balance_header(&self, source_sizes: &[(String, u64, usize)], trimmed_mean_bytes: u64) {
        let num_sources = source_sizes.len();
        let trim_count = ((num_sources as f64 * TRIM_PERCENTAGE).floor() as usize / 2) * 2;
        let trim_each_side = trim_count / 2;
        let trimmed_mean_mb = trimmed_mean_bytes as f64 / BYTES_TO_MB;

        println!("\n=== Corpus Balancing (Trimmed Mean) ===");
        println!("Total sources: {}", num_sources);
        if trim_each_side > 0 {
            let sizes_only: Vec<u64> = source_sizes.iter().map(|(_, size, _)| *size).collect();
            let trimmed_count = sizes_only.len() - trim_count;
            println!("Trimmed {} sources from each end ({} total)", trim_each_side, trim_count);
            println!("Calculating mean from {} middle sources", trimmed_count);
        }
        println!("Target size per source: {:.2} MB (trimmed mean)", trimmed_mean_mb);
        println!("\nPer-source selection:");
    }

    fn perform_balancing(
        &self,
        source_files: &HashMap<String, Vec<(PathBuf, u64)>>,
        source_sizes: &[(String, u64, usize)],
        trimmed_mean_bytes: u64,
    ) -> Vec<PathBuf> {
        use rand::{
            rng,
            seq::SliceRandom,
        };

        let mut rng = rng();
        let mut balanced_files = Vec::new();

        for (source_name, total_size, file_count) in source_sizes {
            let files = source_files.get(source_name).unwrap();
            let size_mb = *total_size as f64 / BYTES_TO_MB;

            if *total_size <= trimmed_mean_bytes {
                // Include all files from sources at or below trimmed mean
                for (path, _) in files {
                    balanced_files.push(path.clone());
                }
            } else {
                // Randomly sample files until we reach approximately the trimmed mean
                let mut shuffled = files.clone();
                shuffled.shuffle(&mut rng);

                let mut accumulated_size = 0u64;
                let mut selected_count = 0;

                for (path, size) in shuffled {
                    if accumulated_size >= trimmed_mean_bytes {
                        break;
                    }
                    balanced_files.push(path);
                    accumulated_size += size;
                    selected_count += 1;
                }

                println!(
                    "  {}: {:.2} MB ({} files) → {:.2} MB ({} files, randomly sampled)",
                    source_name,
                    size_mb,
                    file_count,
                    accumulated_size as f64 / BYTES_TO_MB,
                    selected_count
                );
            }
        }

        balanced_files
    }

    fn log_balance_summary(&self, balanced_files: &[PathBuf], original_count: usize) {
        println!(
            "\nTotal files after balancing: {} (was {})",
            balanced_files.len(),
            original_count
        );
        println!("========================\n");
    }
}

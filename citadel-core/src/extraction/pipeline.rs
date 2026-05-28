use super::source_file::SourceFile;
use super::file_reader::FileReader;
use super::strategy::ExtractionStrategy;
use super::ExtractionResult;

/// Pipeline facade: FileReader → ExtractionStrategy → (results, hashes)
///
/// Es la unica entrada que necesita el napi bridge.
/// Cambiar de secuencial a paralelo es solo cambiar la estrategia.
pub struct ExtractionPipeline {
    strategy: Box<dyn ExtractionStrategy>,
}

impl ExtractionPipeline {
    pub fn new(strategy: Box<dyn ExtractionStrategy>) -> Self {
        ExtractionPipeline { strategy }
    }

    pub fn run(
        &self,
        paths: &[String],
        root_dir: &str,
        framework_names: &[String],
    ) -> (Vec<ExtractionResult>, Vec<String>) {
        let files: Vec<SourceFile> = FileReader::read_all(paths, root_dir);
        let hashes: Vec<String> = files.iter().map(|f| f.sha256.clone()).collect();
        let results = self.strategy.execute(&files, framework_names);
        (results, hashes)
    }
}

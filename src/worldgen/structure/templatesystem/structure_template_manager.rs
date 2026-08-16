use std::fs;
use std::io::{Cursor, Read as _};
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use flate2::read::GzDecoder;
use simdnbt::owned::{Nbt, read as read_nbt};
use parking_lot::RwLock;
use rustc_hash::FxHashMap;

use crate::util::datapack::{collect_json_files, path_to_id, read_and_parse_normalized};
use crate::util::ident::Identifier;
use crate::worldgen::structure::templatesystem::structure_processor::StructureProcessorList;
use crate::worldgen::structure::templatesystem::structure_template::StructureTemplate;

pub struct StructureTemplateManager {
    directory: PathBuf,
    cache: RwLock<FxHashMap<Identifier, &'static StructureTemplate>>,
}

impl StructureTemplateManager {
    pub fn new(directory: PathBuf) -> Self {
        Self {
            directory,
            cache: RwLock::new(FxHashMap::default()),
        }
    }

    pub fn get_or_create(&self, key: &Identifier) -> &'static StructureTemplate {
        if let Some(template) = self.cache.read().get(key).copied() {
            return template;
        }

        let template: &'static StructureTemplate =
            Box::leak(Box::new(self.load(key).unwrap_or_else(StructureTemplate::empty)));

        let mut cache = self.cache.write();
        cache.entry(key.clone()).or_insert(template)
    }

    fn load(&self, key: &Identifier) -> Option<StructureTemplate> {
        let path = self.directory.join(format!("{}.nbt", key.path));
        let compressed = fs::read(path).ok()?;
        let mut decoder = GzDecoder::new(&compressed[..]);
        let mut decompressed = Vec::new();
        decoder.read_to_end(&mut decompressed).ok()?;
        let Nbt::Some(nbt) = read_nbt(&mut Cursor::new(&decompressed[..])).ok()? else {
            return None;
        };
        Some(StructureTemplate::load(&nbt.as_compound()))
    }
}

static TEMPLATES: LazyLock<StructureTemplateManager> =
    LazyLock::new(|| StructureTemplateManager::new(vanilla_structure_dir()));

pub fn templates() -> &'static StructureTemplateManager {
    &TEMPLATES
}

static PROCESSOR_LISTS: LazyLock<FxHashMap<Identifier, StructureProcessorList>> =
    LazyLock::new(|| load_processor_lists(&vanilla_processor_list_dir()));

pub fn processor_list(key: &Identifier) -> Option<&'static StructureProcessorList> {
    PROCESSOR_LISTS.get(key)
}

fn vanilla_structure_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/datapack/structure")
}

fn vanilla_processor_list_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/datapack/worldgen/processor_list")
}

fn load_processor_lists(directory: &Path) -> FxHashMap<Identifier, StructureProcessorList> {
    collect_json_files(directory)
        .into_iter()
        .map(|file| {
            let key: Identifier = path_to_id("minecraft", &file, directory)
                .parse()
                .expect("processor list path should be a valid identifier");
            let list: StructureProcessorList = read_and_parse_normalized(&file)
                .unwrap_or_else(|error| panic!("failed to load {}: {error}", file.display()));
            (key, list)
        })
        .collect()
}

use std::fs;
use std::io::{Cursor, Read as _};
use std::sync::LazyLock;

use flate2::read::GzDecoder;
use parking_lot::RwLock;
use rustc_hash::FxHashMap;
use shulkr::util::Key;
use simdnbt::owned::{Nbt, read as read_nbt};

use crate::datapack::{self, Datapack};
use crate::datapack::{collect_json_files, path_to_id, read_and_parse_normalized};
use crate::world::worldgen::structure::templatesystem::structure_processor::StructureProcessorList;
use crate::world::worldgen::structure::templatesystem::structure_template::StructureTemplate;

pub struct StructureTemplateManager {
    pack: &'static Datapack,
    cache: RwLock<FxHashMap<Key, &'static StructureTemplate>>,
}

impl StructureTemplateManager {
    pub fn new(pack: &'static Datapack) -> Self {
        Self {
            pack,
            cache: RwLock::new(FxHashMap::default()),
        }
    }

    pub fn get_or_create(&self, key: &Key) -> &'static StructureTemplate {
        if let Some(template) = self.cache.read().get(key).copied() {
            return template;
        }

        let template: &'static StructureTemplate = Box::leak(Box::new(
            self.load(key).unwrap_or_else(StructureTemplate::empty),
        ));

        let mut cache = self.cache.write();
        cache.entry(key.clone()).or_insert(template)
    }

    fn load(&self, key: &Key) -> Option<StructureTemplate> {
        let path = self
            .pack
            .files(key.namespace(), &format!("structure/{}.nbt", key.path()))
            .pop()?;
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
    LazyLock::new(|| StructureTemplateManager::new(datapack::active()));

pub fn templates() -> &'static StructureTemplateManager {
    &TEMPLATES
}

static PROCESSOR_LISTS: LazyLock<FxHashMap<Key, StructureProcessorList>> =
    LazyLock::new(|| load_processor_lists(datapack::active()));

pub fn processor_list(key: &Key) -> Option<&'static StructureProcessorList> {
    PROCESSOR_LISTS.get(key)
}

fn load_processor_lists(pack: &Datapack) -> FxHashMap<Key, StructureProcessorList> {
    let mut lists = FxHashMap::default();
    for (namespace, directory) in pack.namespaced_dirs("worldgen/processor_list") {
        for file in collect_json_files(&directory) {
            let key = Key::of(path_to_id(&namespace, &file, &directory));
            let list: StructureProcessorList = read_and_parse_normalized(&file)
                .unwrap_or_else(|error| panic!("failed to load {}: {error}", file.display()));
            lists.insert(key, list);
        }
    }
    lists
}

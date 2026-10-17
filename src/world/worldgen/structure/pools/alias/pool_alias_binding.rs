use shulkr::util::Key;

use crate::util::rng::RandomSource;

#[derive(Debug, Clone)]
pub struct WeightedTarget<T> {
    pub value: T,
    pub weight: i32,
}

#[derive(Debug, Clone)]
pub enum PoolAliasBinding {
    Direct {
        alias: Key,
        target: Key,
    },
    Random {
        alias: Key,
        targets: Vec<WeightedTarget<Key>>,
    },
    RandomGroup {
        groups: Vec<WeightedTarget<Vec<PoolAliasBinding>>>,
    },
}

impl PoolAliasBinding {
    pub fn for_each_resolved(&self, random: &mut RandomSource, consumer: &mut dyn FnMut(Key, Key)) {
        match self {
            Self::Direct { alias, target } => consumer(alias.clone(), target.clone()),
            Self::Random { alias, targets } => {
                if let Some(target) = pick_random(targets, random) {
                    consumer(alias.clone(), target.clone());
                }
            }
            Self::RandomGroup { groups } => {
                if let Some(group) = pick_random(groups, random) {
                    let group = group.clone();
                    for binding in &group {
                        binding.for_each_resolved(random, consumer);
                    }
                }
            }
        }
    }
}

fn pick_random<'a, T>(
    entries: &'a [WeightedTarget<T>],
    random: &mut RandomSource,
) -> Option<&'a T> {
    let total: i32 = entries.iter().map(|entry| entry.weight).sum();
    if total <= 0 {
        return None;
    }
    let mut roll = random.next_i32_bounded(total);
    for entry in entries {
        roll -= entry.weight;
        if roll < 0 {
            return Some(&entry.value);
        }
    }
    None
}

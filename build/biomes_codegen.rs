#![expect(
    clippy::unwrap_used,
    reason = "build script transforms extracted vanilla data and favors direct generation code over pedantic reshaping"
)]

use std::fs;

use heck::ToShoutySnakeCase;
use proc_macro2::{Ident, Span, TokenStream};
use quote::quote;
use serde::{Deserialize, Deserializer};

#[derive(Deserialize)]
struct BiomeJson {
    has_precipitation: bool,
    temperature: f32,
    #[serde(default)]
    temperature_modifier: Option<String>,
    #[serde(deserialize_with = "one_or_many")]
    carvers: Vec<String>,
    features: Vec<Vec<String>>,
}

fn one_or_many<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<String>, D::Error> {
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum OneOrMany {
        One(String),
        Many(Vec<String>),
    }
    Ok(match OneOrMany::deserialize(d)? {
        OneOrMany::One(s) => vec![s],
        OneOrMany::Many(v) => v,
    })
}

fn strip_namespace(id: &str) -> &str {
    id.strip_prefix("minecraft:").unwrap_or(id)
}

fn identifier_tokens(id: &str) -> TokenStream {
    let path = strip_namespace(id);
    quote! { Identifier::const_vanilla(#path) }
}

pub fn build() -> TokenStream {
    println!("cargo:rerun-if-changed=assets/biome");

    let mut entries: Vec<(String, String)> = fs::read_dir("assets/biome")
        .unwrap()
        .map(|entry| {
            let path = entry.unwrap().path();
            let name = path
                .file_stem()
                .unwrap()
                .to_str()
                .unwrap()
                .to_owned();
            let content = fs::read_to_string(&path).unwrap();
            (name, content)
        })
        .collect();
    entries.sort_by(|a, b| a.0.cmp(&b.0));

    let mut consts = TokenStream::new();
    let mut register_stream = TokenStream::new();

    for (name, content) in &entries {
        let biome: BiomeJson = serde_json::from_str(content).unwrap();

        let const_name = Ident::new(&name.to_shouty_snake_case(), Span::call_site());
        let carvers_name = Ident::new(&format!("{}_CARVERS", name.to_shouty_snake_case()), Span::call_site());
        let features_name = Ident::new(&format!("{}_FEATURES", name.to_shouty_snake_case()), Span::call_site());

        let carver_tokens: Vec<TokenStream> =
            biome.carvers.iter().map(|id| identifier_tokens(id)).collect();

        let mut step_names = Vec::new();
        let mut step_consts = TokenStream::new();
        for (i, step) in biome.features.iter().enumerate() {
            let step_name = Ident::new(
                &format!("{}_FEATURES_STEP_{i}", name.to_shouty_snake_case()),
                Span::call_site(),
            );
            let feature_tokens: Vec<TokenStream> =
                step.iter().map(|id| identifier_tokens(id)).collect();
            step_consts.extend(quote! {
                const #step_name: &[Identifier] = &[#(#feature_tokens),*];
            });
            step_names.push(step_name);
        }

        let temperature_modifier = match biome.temperature_modifier.as_deref() {
            Some("frozen") => quote! { TemperatureModifier::Frozen },
            Some(other) => panic!("unknown temperature modifier: {other}"),
            None => quote! { TemperatureModifier::None },
        };

        let has_precipitation = biome.has_precipitation;
        let temperature = biome.temperature;

        consts.extend(quote! {
            const #carvers_name: &[Identifier] = &[#(#carver_tokens),*];
            #step_consts
            const #features_name: &[&[Identifier]] = &[#(#step_names),*];

            pub static #const_name: Biome = Biome {
                key: Identifier::const_vanilla(#name),
                has_precipitation: #has_precipitation,
                temperature: #temperature,
                temperature_modifier: #temperature_modifier,
                carvers: #carvers_name,
                features: #features_name,
                id: OnceLock::new(),
            };
        });

        register_stream.extend(quote! {
            registry.register(&#const_name);
        });
    }

    quote! {
        use std::sync::OnceLock;

        use crate::registry::biome::{Biome, BiomeRegistry, TemperatureModifier};
        use crate::util::ident::Identifier;

        #consts

        pub fn register_biomes(registry: &mut BiomeRegistry) {
            #register_stream
        }
    }
}

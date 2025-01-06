use std::path::{Path, PathBuf};
use std::str::FromStr;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use strum::IntoEnumIterator;
use strum_macros::EnumIter;

use lindera_dictionary::dictionary_builder::cc_cedict::CcCedictBuilder;
use lindera_dictionary::dictionary_builder::ipadic::IpadicBuilder;
use lindera_dictionary::dictionary_builder::ipadic_neologd::IpadicNeologdBuilder;
use lindera_dictionary::dictionary_builder::ko_dic::KoDicBuilder;
use lindera_dictionary::dictionary_builder::unidic::UnidicBuilder;
use lindera_dictionary::dictionary_builder::DictionaryBuilder;
use lindera_dictionary::dictionary_loader::character_definition::CharacterDefinitionLoader;
use lindera_dictionary::dictionary_loader::connection_cost_matrix::ConnectionCostMatrixLoader;
use lindera_dictionary::dictionary_loader::prefix_dictionary::PrefixDictionaryLoader;
use lindera_dictionary::dictionary_loader::unknown_dictionary::UnknownDictionaryLoader;
use lindera_dictionary::util::read_file;

use crate::error::{LinderaError, LinderaErrorKind};
use crate::LinderaResult;

pub type Dictionary = lindera_dictionary::dictionary::Dictionary;
pub type UserDictionary = lindera_dictionary::dictionary::UserDictionary;
pub type WordId = lindera_dictionary::viterbi::WordId;

#[derive(Debug, Clone, EnumIter, Deserialize, Serialize, PartialEq, Eq)]
pub enum DictionaryKind {
    #[serde(rename = "ipadic")]
    IPADIC,
    #[serde(rename = "ipadic-neologd")]
    IPADICNEologd,
    #[serde(rename = "unidic")]
    UniDic,
    #[serde(rename = "ko-dic")]
    KoDic,
    #[serde(rename = "cc-cedict")]
    CcCedict,
}

impl DictionaryKind {
    pub fn variants() -> Vec<DictionaryKind> {
        DictionaryKind::iter().collect::<Vec<_>>()
    }

    pub fn contained_variants() -> Vec<DictionaryKind> {
        DictionaryKind::variants()
            .into_iter()
            .filter(|kind| match kind {
                DictionaryKind::IPADIC => cfg!(feature = "ipadic"),
                DictionaryKind::IPADICNEologd => cfg!(feature = "ipadic-neologd"),
                DictionaryKind::UniDic => cfg!(feature = "unidic"),
                DictionaryKind::KoDic => cfg!(feature = "ko-dic"),
                DictionaryKind::CcCedict => cfg!(feature = "cc-cedict"),
            })
            .collect::<Vec<_>>()
    }

    pub fn as_str(&self) -> &str {
        match self {
            DictionaryKind::IPADIC => "ipadic",
            DictionaryKind::IPADICNEologd => "ipadic-neologd",
            DictionaryKind::UniDic => "unidic",
            DictionaryKind::KoDic => "ko-dic",
            DictionaryKind::CcCedict => "cc-cedict",
        }
    }
}

impl FromStr for DictionaryKind {
    type Err = LinderaError;
    fn from_str(input: &str) -> Result<DictionaryKind, Self::Err> {
        match input {
            "ipadic" => Ok(DictionaryKind::IPADIC),
            "ipadic-neologd" => Ok(DictionaryKind::IPADICNEologd),
            "unidic" => Ok(DictionaryKind::UniDic),
            "ko-dic" => Ok(DictionaryKind::KoDic),
            "cc-cedict" => Ok(DictionaryKind::CcCedict),
            _ => Err(LinderaErrorKind::Dictionary
                .with_error(anyhow::anyhow!("Invalid dictionary kind: {}", input))),
        }
    }
}

pub type DictionaryConfig = Value;
pub type UserDictionaryConfig = Value;

pub fn resolve_builder(
    dictionary_type: DictionaryKind,
) -> LinderaResult<Box<dyn DictionaryBuilder>> {
    match dictionary_type {
        DictionaryKind::IPADIC => Ok(Box::new(IpadicBuilder::new())),
        DictionaryKind::IPADICNEologd => Ok(Box::new(IpadicNeologdBuilder::new())),
        DictionaryKind::UniDic => Ok(Box::new(UnidicBuilder::new())),
        DictionaryKind::KoDic => Ok(Box::new(KoDicBuilder::new())),
        DictionaryKind::CcCedict => Ok(Box::new(CcCedictBuilder::new())),
    }
}

pub fn load_dictionary_from_path(path: &Path) -> LinderaResult<Dictionary> {
    Ok(Dictionary {
        prefix_dictionary: PrefixDictionaryLoader::load(path)?,
        connection_cost_matrix: ConnectionCostMatrixLoader::load(path)?,
        character_definition: CharacterDefinitionLoader::load(path)?,
        unknown_dictionary: UnknownDictionaryLoader::load(path)?,
    })
}

#[cfg(feature = "memmap")]
pub fn load_dictionary_from_path_memmap(path: &Path) -> LinderaResult<Dictionary> {
    Ok(Dictionary {
        prefix_dictionary: PrefixDictionaryLoader::load_memmap(path)?,
        connection_cost_matrix: ConnectionCostMatrixLoader::load_memmap(path)?,
        character_definition: CharacterDefinitionLoader::load(path)?,
        unknown_dictionary: UnknownDictionaryLoader::load(path)?,
    })
}

pub fn load_dictionary_from_kind(kind: DictionaryKind) -> LinderaResult<Dictionary> {
    // The dictionary specified by the feature flag will be loaded.
    match kind {
        #[cfg(feature = "ipadic")]
        DictionaryKind::IPADIC => {
            lindera_ipadic::ipadic::load().map_err(|e| LinderaErrorKind::NotFound.with_error(e))
        }
        #[cfg(feature = "ipadic-neologd")]
        DictionaryKind::IPADICNEologd => lindera_ipadic_neologd::ipadic_neologd::load()
            .map_err(|e| LinderaErrorKind::NotFound.with_error(e)),
        #[cfg(feature = "unidic")]
        DictionaryKind::UniDic => {
            lindera_unidic::unidic::load().map_err(|e| LinderaErrorKind::NotFound.with_error(e))
        }
        #[cfg(feature = "ko-dic")]
        DictionaryKind::KoDic => {
            lindera_ko_dic::ko_dic::load().map_err(|e| LinderaErrorKind::NotFound.with_error(e))
        }
        #[cfg(feature = "cc-cedict")]
        DictionaryKind::CcCedict => lindera_cc_cedict::cc_cedict::load()
            .map_err(|e| LinderaErrorKind::NotFound.with_error(e)),
        #[allow(unreachable_patterns)]
        _ => Err(LinderaErrorKind::Args
            .with_error(anyhow::anyhow!("Invalid dictionary type: {:?}", kind))),
    }
}

pub fn load_dictionary_from_config(
    dictionary_config: &DictionaryConfig,
) -> LinderaResult<Dictionary> {
    match dictionary_config.get("kind") {
        Some(kind_value) => {
            let kind = DictionaryKind::from_str(kind_value.as_str().ok_or_else(|| {
                LinderaErrorKind::Parse.with_error(anyhow::anyhow!("kind field must be a string"))
            })?)?;

            // Load contained dictionary from kind value in config.
            load_dictionary_from_kind(kind)
        }
        None => {
            match dictionary_config.get("path") {
                Some(path_value) => {
                    let path = PathBuf::from(path_value.as_str().ok_or_else(|| {
                        LinderaErrorKind::Parse
                            .with_error(anyhow::anyhow!("path field must be a string"))
                    })?);

                    // load external dictionary from path
                    if dictionary_config
                        .get("memmap")
                        .is_some_and(|x| x.as_bool().is_some_and(|b| b))
                    {
                        #[cfg(feature = "memmap")]
                        {
                            load_dictionary_from_path_memmap(path.as_path())
                        }
                        #[cfg(not(feature = "memmap"))]
                        {
                            // note: warn about this?
                            load_dictionary_from_path(path.as_path())
                        }
                    } else {
                        load_dictionary_from_path(path.as_path())
                    }
                }
                None => Err(LinderaErrorKind::Args.with_error(anyhow::anyhow!(
                    "kind field or path field must be specified"
                ))),
            }
        }
    }
}

pub fn load_user_dictionary_from_csv(
    kind: DictionaryKind,
    path: &Path,
) -> LinderaResult<UserDictionary> {
    let builder = resolve_builder(kind)?;
    builder
        .build_user_dict(path)
        .map_err(|err| LinderaErrorKind::Build.with_error(err))
}

pub fn load_user_dictionary_from_bin(path: &Path) -> LinderaResult<UserDictionary> {
    UserDictionary::load(&read_file(path)?)
}

pub fn load_user_dictionary_from_config(
    dictionary_config: &UserDictionaryConfig,
) -> LinderaResult<UserDictionary> {
    match dictionary_config.get("path") {
        Some(path_value) => {
            let path = PathBuf::from(path_value.as_str().ok_or_else(|| {
                LinderaErrorKind::Parse.with_error(anyhow::anyhow!("path field must be a string"))
            })?);

            match path.extension() {
                Some(ext) => match ext.to_str() {
                    Some("csv") => match dictionary_config.get("kind") {
                        Some(kind_value) => {
                            let kind = DictionaryKind::from_str(kind_value.as_str().ok_or_else(
                                || {
                                    LinderaErrorKind::Parse
                                        .with_error(anyhow::anyhow!("kind field must be a string"))
                                },
                            )?)?;

                            load_user_dictionary_from_csv(kind, path.as_path())
                        }
                        None => Err(LinderaErrorKind::Args.with_error(anyhow::anyhow!(
                            "kind field must be specified if CSV file specified"
                        ))),
                    },
                    Some("bin") => load_user_dictionary_from_bin(path.as_path()),
                    _ => Err(LinderaErrorKind::Args.with_error(anyhow::anyhow!(
                        "Invalid user dictionary source file extension"
                    ))),
                },
                None => Err(LinderaErrorKind::Args
                    .with_error(anyhow::anyhow!("Invalid user dictionary source file"))),
            }
        }
        None => Err(LinderaErrorKind::Args.with_error(anyhow::anyhow!(
            "path field must be specified in user dictionary config"
        ))),
    }
}

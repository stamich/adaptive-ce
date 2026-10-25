//! Block size and storage-engine options (ACE 0.8.0): schemas, segmentation and dictionaries,
//! and `train`.

use std::io::Write;

use ace::{Column, ConfigBuilder, Dictionary, Schema, Segmentation};
use anyhow::{bail, Context, Result};

use crate::args::StorageArgs;
use crate::files;

/// Parses the `--schema` syntax: comma-separated `u8 u16 u32 u64`, `i8 i16 i32 i64`, `f32 f64`
/// and `bN` fields (`N` opaque bytes), for example `u64,f64,u32,b4`.
///
/// # Errors
///
/// A message naming the field that is not understood, or the schema error of the library (no
/// fields, a record larger than 32 bits).
pub fn parse_schema(spec: &str) -> Result<Schema> {
    let mut columns = Vec::new();
    for field in spec.split(',').map(str::trim) {
        let column = match field {
            "u8" => Column::unsigned(1),
            "u16" => Column::unsigned(2),
            "u32" => Column::unsigned(4),
            "u64" => Column::unsigned(8),
            "i8" => Column::signed(1),
            "i16" => Column::signed(2),
            "i32" => Column::signed(4),
            "i64" => Column::signed(8),
            "f32" => Column::float(4),
            "f64" => Column::float(8),
            other => {
                let width = other
                    .strip_prefix('b')
                    .and_then(|digits| digits.parse::<u16>().ok())
                    .filter(|&width| width > 0);
                match width {
                    Some(width) => Column::bytes(width),
                    None => bail!(
                        "unknown schema field {other:?} (use u8 u16 u32 u64, i8 i16 i32 i64, f32 f64 or bN)"
                    ),
                }
            }
        };
        columns.push(column);
    }
    Ok(Schema::new(columns)?)
}

/// Applies `--block-size`, `--schema`, `--segments` and `--dictionary` to `builder`.
///
/// # Errors
///
/// The errors of [`parse_schema`] and of reading the dictionary file.
pub fn storage_options(
    mut builder: ConfigBuilder,
    storage: &StorageArgs,
    dictionary: Option<&str>,
    external: bool,
) -> Result<ConfigBuilder> {
    if let Some(bytes) = storage.block_size {
        builder = builder.block_size(bytes);
    }
    if let Some(spec) = &storage.schema {
        builder = builder.schema(parse_schema(spec).context("--schema")?);
    }
    if storage.segments {
        builder = builder.segmentation(Segmentation::default());
    }
    if let Some(path) = dictionary {
        let dictionary = load_dictionary(path)?;
        builder = if external {
            builder.external_dictionary(dictionary)
        } else {
            builder.dictionary(dictionary)
        };
    }
    Ok(builder)
}

/// Reads one dictionary file (the raw dictionary bytes written by `ace train`).
fn load_dictionary(path: &str) -> Result<Dictionary> {
    Dictionary::new(files::read(path)?).with_context(|| format!("dictionary {path}"))
}

/// Reads the dictionary files named by repeated `--dictionary` options.
///
/// # Errors
///
/// The file is missing, empty or longer than 65 535 bytes.
pub fn load_dictionaries(paths: &[String]) -> Result<Vec<Dictionary>> {
    paths.iter().map(|path| load_dictionary(path)).collect()
}

/// `train`: builds a dictionary from the sample files and writes it to `output`.
///
/// Every file is one sample; with `chunk_size` every file is cut into samples of that many
/// bytes (the last one may be shorter), which lets a few large files train a dictionary.
///
/// # Errors
///
/// A sample file cannot be read, `chunk_size` is zero, or training fails (too few samples,
/// samples that share no content, a size outside 64 to 65 535).
pub fn train(
    output: &str,
    sample_files: &[String],
    size: usize,
    chunk_size: Option<usize>,
) -> Result<()> {
    if chunk_size == Some(0) {
        bail!("--chunk-size must be at least 1");
    }
    let mut samples: Vec<Vec<u8>> = Vec::new();
    for path in sample_files {
        let bytes = files::read(path)?;
        match chunk_size {
            Some(chunk) => samples.extend(bytes.chunks(chunk).map(<[u8]>::to_vec)),
            None => samples.push(bytes),
        }
    }
    let dictionary = Dictionary::train(&samples, size)?;
    files::write(output, dictionary.as_bytes())?;
    writeln!(
        std::io::stdout().lock(),
        "trained dictionary {output}: {} bytes from {} samples id={:016x}",
        dictionary.len(),
        samples.len(),
        dictionary.id()
    )?;
    Ok(())
}

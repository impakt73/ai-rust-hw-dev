use serde::Deserialize;
use std::collections::BTreeSet;
use std::ffi::OsStr;
use std::fmt::{Display, Formatter};
use std::fs::{self, File};
use std::io::{self, BufReader, BufWriter, Read, Write};
use std::path::{Path, PathBuf};
use tempfile::TempDir;
use walkdir::WalkDir;
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipWriter};

const CORE_ROOT_DIR: &str = "Cores";
const ROOT_STAGE_DIRECTORIES: &[&str] = &["Assets", "Platforms", "Presets"];
const CORE_FILE_EXTENSIONS: &[&str] = &["bin", "json", "txt"];

#[derive(Debug)]
pub enum PacketError {
    Io(io::Error),
    Json(serde_json::Error),
    WalkDir(walkdir::Error),
    Zip(zip::result::ZipError),
    MissingCoreDefinition(PathBuf),
    MissingBitstreamDefinition,
    MultipleBitstreamDefinitions(Vec<String>),
    InvalidBitstreamFilename(String),
    InvalidMetadataField { field: &'static str, value: String },
    InvalidOutputPath(PathBuf),
    UnexpectedOutputFilename { expected: String, actual: String },
}

impl Display for PacketError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(error) => write!(f, "I/O error: {error}"),
            Self::Json(error) => write!(f, "JSON parse error: {error}"),
            Self::WalkDir(error) => write!(f, "directory walk error: {error}"),
            Self::Zip(error) => write!(f, "zip error: {error}"),
            Self::MissingCoreDefinition(path) => {
                write!(f, "core definition file not found: {}", path.display())
            }
            Self::MissingBitstreamDefinition => {
                write!(f, "core.json does not define any output bitstream filename")
            }
            Self::MultipleBitstreamDefinitions(filenames) => write!(
                f,
                "core.json defines multiple unique bitstream filenames, but pkt only accepts one input bitstream: {}",
                filenames.join(", ")
            ),
            Self::InvalidBitstreamFilename(filename) => write!(
                f,
                "bitstream filename must end with .rbf_r for Pocket packaging: {filename}"
            ),
            Self::InvalidMetadataField { field, value } => write!(
                f,
                "core.json metadata field {field} contains an invalid value for packaging paths: {value}"
            ),
            Self::InvalidOutputPath(path) => {
                write!(f, "output path must be a directory or a .zip file: {}", path.display())
            }
            Self::UnexpectedOutputFilename { expected, actual } => write!(
                f,
                "output zip filename must match the official Analogue naming convention: expected {expected}, got {actual}"
            ),
        }
    }
}

impl std::error::Error for PacketError {}

impl From<io::Error> for PacketError {
    fn from(value: io::Error) -> Self {
        Self::Io(value)
    }
}

impl From<serde_json::Error> for PacketError {
    fn from(value: serde_json::Error) -> Self {
        Self::Json(value)
    }
}

impl From<walkdir::Error> for PacketError {
    fn from(value: walkdir::Error) -> Self {
        Self::WalkDir(value)
    }
}

impl From<zip::result::ZipError> for PacketError {
    fn from(value: zip::result::ZipError) -> Self {
        Self::Zip(value)
    }
}

#[derive(Debug, Deserialize)]
struct CoreDefinitionFile {
    core: CoreDefinition,
}

#[derive(Debug, Deserialize)]
struct CoreDefinition {
    metadata: CoreMetadata,
    cores: Vec<CoreBitstream>,
}

#[derive(Debug, Deserialize)]
struct CoreMetadata {
    author: String,
    shortname: String,
    version: String,
    date_release: String,
}

#[derive(Debug, Deserialize)]
struct CoreBitstream {
    filename: String,
}

pub fn package_core(
    input_rbf: &Path,
    core_source: &Path,
    output_path: &Path,
) -> Result<PathBuf, PacketError> {
    let core_definition_path = core_source.join("core.json");
    if !core_definition_path.is_file() {
        return Err(PacketError::MissingCoreDefinition(core_definition_path));
    }

    let core_definition = read_core_definition(&core_definition_path)?;
    validate_metadata(&core_definition.metadata)?;

    let core_folder_name = format!(
        "{}.{}",
        core_definition.metadata.author, core_definition.metadata.shortname
    );
    let zip_file_name = format!(
        "{}_{}_{}.zip",
        core_folder_name, core_definition.metadata.version, core_definition.metadata.date_release
    );
    let output_zip_path = resolve_output_zip_path(output_path, &zip_file_name)?;

    if let Some(parent) = output_zip_path.parent() {
        fs::create_dir_all(parent)?;
    }

    let bitstream_name = get_bitstream_filename(&core_definition.cores)?;
    let staging_dir = tempfile::tempdir()?;
    let staged_core_dir = staging_dir
        .path()
        .join(CORE_ROOT_DIR)
        .join(&core_folder_name);
    fs::create_dir_all(&staged_core_dir)?;

    copy_core_files(core_source, &staged_core_dir)?;
    copy_supported_root_directories(core_source, staging_dir.path())?;
    reverse_bitstream(input_rbf, &staged_core_dir.join(bitstream_name))?;
    write_zip(staging_dir, &output_zip_path)?;

    Ok(output_zip_path)
}

fn read_core_definition(path: &Path) -> Result<CoreDefinition, PacketError> {
    let file = File::open(path)?;
    let reader = BufReader::new(file);
    let definition = serde_json::from_reader::<_, CoreDefinitionFile>(reader)?;
    Ok(definition.core)
}

fn validate_metadata(metadata: &CoreMetadata) -> Result<(), PacketError> {
    for (field, value) in [
        ("author", metadata.author.as_str()),
        ("shortname", metadata.shortname.as_str()),
        ("version", metadata.version.as_str()),
        ("date_release", metadata.date_release.as_str()),
    ] {
        if value.is_empty() || value.contains('/') || value.contains('\\') {
            return Err(PacketError::InvalidMetadataField {
                field,
                value: value.to_string(),
            });
        }
    }

    Ok(())
}

fn get_bitstream_filename(cores: &[CoreBitstream]) -> Result<&str, PacketError> {
    let filenames = cores
        .iter()
        .map(|core| core.filename.as_str())
        .collect::<BTreeSet<_>>();

    if filenames.is_empty() {
        return Err(PacketError::MissingBitstreamDefinition);
    }

    if filenames.len() > 1 {
        return Err(PacketError::MultipleBitstreamDefinitions(
            filenames.into_iter().map(ToOwned::to_owned).collect(),
        ));
    }

    let filename = filenames
        .into_iter()
        .next()
        .ok_or(PacketError::MissingBitstreamDefinition)?;
    if !filename.ends_with(".rbf_r") {
        return Err(PacketError::InvalidBitstreamFilename(filename.to_string()));
    }

    Ok(filename)
}

fn resolve_output_zip_path(
    output_path: &Path,
    zip_file_name: &str,
) -> Result<PathBuf, PacketError> {
    if output_path.exists() && output_path.is_dir() {
        return Ok(output_path.join(zip_file_name));
    }

    if output_path.extension() == Some(OsStr::new("zip")) {
        let actual = output_path
            .file_name()
            .and_then(OsStr::to_str)
            .ok_or_else(|| PacketError::InvalidOutputPath(output_path.to_path_buf()))?;
        if actual != zip_file_name {
            return Err(PacketError::UnexpectedOutputFilename {
                expected: zip_file_name.to_string(),
                actual: actual.to_string(),
            });
        }

        return Ok(output_path.to_path_buf());
    }

    if output_path.extension().is_none() {
        return Ok(output_path.join(zip_file_name));
    }

    Err(PacketError::InvalidOutputPath(output_path.to_path_buf()))
}

fn copy_core_files(core_source: &Path, staged_core_dir: &Path) -> Result<(), PacketError> {
    for entry in fs::read_dir(core_source)? {
        let entry = entry?;
        let path = entry.path();
        if !path.is_file() || is_hidden_path(&path) || !should_copy_to_core_dir(&path) {
            continue;
        }

        let filename = path
            .file_name()
            .ok_or_else(|| PacketError::InvalidOutputPath(path.clone()))?;
        fs::copy(&path, staged_core_dir.join(filename))?;
    }

    Ok(())
}

fn copy_supported_root_directories(
    core_source: &Path,
    staging_root: &Path,
) -> Result<(), PacketError> {
    for directory_name in ROOT_STAGE_DIRECTORIES {
        let source_root = core_source.join(directory_name);
        if !source_root.is_dir() {
            continue;
        }

        for entry in WalkDir::new(&source_root) {
            let entry = entry?;
            let relative_path = entry
                .path()
                .strip_prefix(core_source)
                .map_err(|_| PacketError::InvalidOutputPath(entry.path().to_path_buf()))?;
            let destination = staging_root.join(relative_path);

            if entry.file_type().is_dir() {
                fs::create_dir_all(&destination)?;
            } else if entry.file_type().is_file() {
                if let Some(parent) = destination.parent() {
                    fs::create_dir_all(parent)?;
                }
                fs::copy(entry.path(), destination)?;
            }
        }
    }

    Ok(())
}

fn should_copy_to_core_dir(path: &Path) -> bool {
    path.extension()
        .and_then(OsStr::to_str)
        .is_some_and(|extension| CORE_FILE_EXTENSIONS.contains(&extension))
}

fn is_hidden_path(path: &Path) -> bool {
    path.file_name()
        .and_then(OsStr::to_str)
        .is_some_and(|name| name.starts_with('.'))
}

fn reverse_bitstream(input_rbf: &Path, output_rbf_r: &Path) -> Result<(), PacketError> {
    let input = File::open(input_rbf)?;
    let output = File::create(output_rbf_r)?;
    let mut reader = BufReader::new(input);
    let mut writer = BufWriter::new(output);
    let mut buffer = [0u8; 8192];

    loop {
        let bytes_read = reader.read(&mut buffer)?;
        if bytes_read == 0 {
            break;
        }

        for byte in &mut buffer[..bytes_read] {
            *byte = reverse_byte(*byte);
        }

        writer.write_all(&buffer[..bytes_read])?;
    }

    writer.flush()?;
    Ok(())
}

fn write_zip(staging_dir: TempDir, output_zip_path: &Path) -> Result<(), PacketError> {
    let zip_file = File::create(output_zip_path)?;
    let mut zip = ZipWriter::new(BufWriter::new(zip_file));
    let file_options = SimpleFileOptions::default()
        .compression_method(CompressionMethod::Deflated)
        .unix_permissions(0o644);
    let directory_options = SimpleFileOptions::default().unix_permissions(0o755);
    let staging_root = staging_dir.path().to_path_buf();

    let mut entries = WalkDir::new(&staging_root)
        .into_iter()
        .collect::<Result<Vec<_>, _>>()?;
    entries.sort_by(|left, right| left.path().cmp(right.path()));

    for entry in entries {
        let path = entry.path();
        if path == staging_root {
            continue;
        }

        let relative = path
            .strip_prefix(&staging_root)
            .map_err(|_| PacketError::InvalidOutputPath(path.to_path_buf()))?;
        let zip_path = relative.to_string_lossy().replace('\\', "/");

        if entry.file_type().is_dir() {
            zip.add_directory(zip_path, directory_options)?;
            continue;
        }

        zip.start_file(zip_path, file_options)?;
        let mut file = BufReader::new(File::open(path)?);
        io::copy(&mut file, &mut zip)?;
    }

    zip.finish()?;
    Ok(())
}

pub fn reverse_byte(byte: u8) -> u8 {
    byte.reverse_bits()
}

#[cfg(test)]
mod tests {
    use super::reverse_byte;

    #[test]
    fn test_reverse_byte() {
        assert_eq!(reverse_byte(0b0000_0001), 0b1000_0000);
        assert_eq!(reverse_byte(0b1000_0000), 0b0000_0001);
        assert_eq!(reverse_byte(0b0011_1100), 0b0011_1100);
    }
}

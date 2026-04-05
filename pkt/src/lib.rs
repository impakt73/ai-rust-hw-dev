use serde::Deserialize;
use std::collections::BTreeSet;
use std::ffi::OsStr;
use std::fmt::{Display, Formatter};
use std::fs::{self, File};
use std::io::{self, BufReader, BufWriter, Read, Write};
use std::path::{Component, Path, PathBuf};
use tempfile::TempDir;
use walkdir::WalkDir;
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipWriter};

const CORE_ROOT_DIR: &str = "Cores";
const ROOT_STAGE_DIRECTORIES: &[&str] = &["Assets", "Platforms", "Presets"];
const CORE_FILE_EXTENSIONS: &[&str] = &["bin", "json", "txt"];
const PLATFORM_PATH_PLACEHOLDER: &str = "ex_platform";
const CORE_PATH_PLACEHOLDER: &str = "ex_core_name";

#[derive(Debug)]
pub enum PackageError {
    Io(io::Error),
    Json(serde_json::Error),
    WalkDir(walkdir::Error),
    Zip(zip::result::ZipError),
    MissingCoreDefinition(PathBuf),
    MissingBitstreamDefinition,
    MultipleBitstreamDefinitions(Vec<String>),
    InvalidBitstreamFilename(String),
    InvalidMetadataField { field: &'static str, value: String },
    MissingPlatformIdForAssetPath(PathBuf),
    InvalidOutputPath(PathBuf),
}

impl Display for PackageError {
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
                "bitstream filename must be a plain filename ending with .rbf_r for Pocket packaging: {filename}"
            ),
            Self::InvalidMetadataField { field, value } => write!(
                f,
                "core.json metadata field {field} contains an invalid value for packaging paths: {value}"
            ),
            Self::MissingPlatformIdForAssetPath(path) => write!(
                f,
                "core.json must define at least one platform_id to package asset path: {}",
                path.display()
            ),
            Self::InvalidOutputPath(path) => {
                write!(f, "output path must be a directory: {}", path.display())
            }
        }
    }
}

impl std::error::Error for PackageError {}

impl From<io::Error> for PackageError {
    fn from(value: io::Error) -> Self {
        Self::Io(value)
    }
}

impl From<serde_json::Error> for PackageError {
    fn from(value: serde_json::Error) -> Self {
        Self::Json(value)
    }
}

impl From<walkdir::Error> for PackageError {
    fn from(value: walkdir::Error) -> Self {
        Self::WalkDir(value)
    }
}

impl From<zip::result::ZipError> for PackageError {
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
    #[serde(default)]
    platform_ids: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct CoreBitstream {
    filename: String,
}

pub fn package_core(
    input_rbf: &Path,
    core_source: &Path,
    output_dir: &Path,
) -> Result<PathBuf, PackageError> {
    let core_definition_path = core_source.join("core.json");
    if !core_definition_path.is_file() {
        return Err(PackageError::MissingCoreDefinition(core_definition_path));
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
    let output_zip_path = resolve_output_zip_path(output_dir, &zip_file_name)?;

    if let Some(parent) = output_zip_path.parent() {
        fs::create_dir_all(parent)?;
    }

    let bitstream_name = get_bitstream_filename(&core_definition.cores)?;
    let packaging_paths = PackagingPaths {
        core_folder_name: core_folder_name.clone(),
        primary_platform_id: core_definition.metadata.platform_ids.first().cloned(),
    };
    let staging_temp_dir = tempfile::tempdir()?;
    let staged_core_dir = staging_temp_dir
        .path()
        .join(CORE_ROOT_DIR)
        .join(&core_folder_name);
    fs::create_dir_all(&staged_core_dir)?;

    copy_core_files(core_source, &staged_core_dir)?;
    copy_supported_root_directories(core_source, staging_temp_dir.path(), &packaging_paths)?;
    reverse_bitstream(input_rbf, &staged_core_dir.join(bitstream_name))?;
    write_zip(staging_temp_dir, &output_zip_path)?;

    Ok(output_zip_path)
}

fn read_core_definition(path: &Path) -> Result<CoreDefinition, PackageError> {
    let file = File::open(path)?;
    let reader = BufReader::new(file);
    let definition = serde_json::from_reader::<_, CoreDefinitionFile>(reader)?;
    Ok(definition.core)
}

fn validate_metadata(metadata: &CoreMetadata) -> Result<(), PackageError> {
    for (field, value) in [
        ("author", metadata.author.as_str()),
        ("shortname", metadata.shortname.as_str()),
        ("version", metadata.version.as_str()),
        ("date_release", metadata.date_release.as_str()),
    ] {
        if value.is_empty() || value.contains('/') || value.contains('\\') {
            return Err(PackageError::InvalidMetadataField {
                field,
                value: value.to_string(),
            });
        }
    }

    for platform_id in &metadata.platform_ids {
        if platform_id.is_empty() || platform_id.contains('/') || platform_id.contains('\\') {
            return Err(PackageError::InvalidMetadataField {
                field: "platform_ids",
                value: platform_id.clone(),
            });
        }
    }

    Ok(())
}

fn get_bitstream_filename(cores: &[CoreBitstream]) -> Result<&str, PackageError> {
    let filenames = cores
        .iter()
        .map(|core| core.filename.as_str())
        .collect::<BTreeSet<_>>();

    if filenames.is_empty() {
        return Err(PackageError::MissingBitstreamDefinition);
    }

    if filenames.len() > 1 {
        return Err(PackageError::MultipleBitstreamDefinitions(
            filenames.into_iter().map(ToOwned::to_owned).collect(),
        ));
    }

    let filename = filenames
        .into_iter()
        .next()
        .ok_or(PackageError::MissingBitstreamDefinition)?;
    if !is_valid_bitstream_filename(filename) {
        return Err(PackageError::InvalidBitstreamFilename(filename.to_string()));
    }

    Ok(filename)
}

fn is_valid_bitstream_filename(filename: &str) -> bool {
    if !filename.ends_with(".rbf_r") || filename.contains('/') || filename.contains('\\') {
        return false;
    }

    let mut components = Path::new(filename).components();
    matches!(components.next(), Some(Component::Normal(_))) && components.next().is_none()
}

fn resolve_output_zip_path(
    output_dir: &Path,
    zip_file_name: &str,
) -> Result<PathBuf, PackageError> {
    if output_dir.exists() && !output_dir.is_dir() {
        return Err(PackageError::InvalidOutputPath(output_dir.to_path_buf()));
    }

    if output_dir.extension() == Some(OsStr::new("zip")) {
        return Err(PackageError::InvalidOutputPath(output_dir.to_path_buf()));
    }

    Ok(output_dir.join(zip_file_name))
}

fn copy_core_files(core_source: &Path, staged_core_dir: &Path) -> Result<(), PackageError> {
    for entry in fs::read_dir(core_source)? {
        let entry = entry?;
        let path = entry.path();
        let file_type = entry.file_type()?;
        if file_type.is_symlink()
            || !file_type.is_file()
            || is_hidden_path(&path)
            || !should_copy_to_core_dir(&path)
        {
            continue;
        }

        let filename = path
            .file_name()
            .ok_or_else(|| PackageError::InvalidOutputPath(path.clone()))?;
        fs::copy(&path, staged_core_dir.join(filename))?;
    }

    Ok(())
}

struct PackagingPaths {
    core_folder_name: String,
    primary_platform_id: Option<String>,
}

fn copy_supported_root_directories(
    core_source: &Path,
    staging_root: &Path,
    packaging_paths: &PackagingPaths,
) -> Result<(), PackageError> {
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
                .map_err(|_| PackageError::InvalidOutputPath(entry.path().to_path_buf()))?;
            let destination =
                staging_root.join(resolve_staged_root_path(relative_path, packaging_paths)?);

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

fn resolve_staged_root_path(
    relative_path: &Path,
    packaging_paths: &PackagingPaths,
) -> Result<PathBuf, PackageError> {
    relative_path
        .components()
        .map(|component| match component {
            Component::Normal(name) if name == OsStr::new(PLATFORM_PATH_PLACEHOLDER) => {
                packaging_paths
                    .primary_platform_id
                    .as_deref()
                    .map(PathBuf::from)
                    .ok_or_else(|| {
                        PackageError::MissingPlatformIdForAssetPath(relative_path.to_path_buf())
                    })
            }
            Component::Normal(name) if name == OsStr::new(CORE_PATH_PLACEHOLDER) => {
                Ok(PathBuf::from(&packaging_paths.core_folder_name))
            }
            other => Ok(PathBuf::from(other.as_os_str())),
        })
        .try_fold(PathBuf::new(), |mut resolved_path, component| {
            resolved_path.push(component?);
            Ok(resolved_path)
        })
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

fn reverse_bitstream(input_rbf: &Path, output_rbf_r: &Path) -> Result<(), PackageError> {
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
            *byte = byte.reverse_bits();
        }

        writer.write_all(&buffer[..bytes_read])?;
    }

    writer.flush()?;
    Ok(())
}

fn write_zip(staging_dir: TempDir, output_zip_path: &Path) -> Result<(), PackageError> {
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
            .map_err(|_| PackageError::InvalidOutputPath(path.to_path_buf()))?;
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

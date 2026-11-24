use clap::{Args, Subcommand};
use precept_ipc::operator::{
    PersonalSyncApplySubmission, PersonalSyncEnvelope, PersonalSyncInvitation,
    PersonalSyncLocalVersion, PersonalSyncPreview, PersonalSyncProfileScope,
    PersonalSyncResolution, PersonalSyncSelection,
};
use precept_ipc::{Error, Location, Result};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    io::{Read, Write},
    path::{Path, PathBuf},
};

const PREPARED_FORMAT: &str = "precept-personal-sync-prepared";
const PREPARED_VERSION: u32 = 1;
const MAX_FILE: u64 = 1024 * 1024;
const MAX_PASSPHRASE_FILE: u64 = 1024;

#[derive(Subcommand)]
pub enum Command {
    Status,
    /// Create this device's sync identity and print its invitation file contents.
    Init {
        #[arg(long)]
        out: Option<PathBuf>,
    },
    /// Print the current invitation and complete-invitation fingerprint.
    Invitation {
        #[arg(long)]
        out: Option<PathBuf>,
    },
    /// Pair an uninitialized device with an invitation verified out of band.
    Pair(TrustArgs),
    /// Trust or re-trust a revoked invitation verified out of band.
    Trust(TrustArgs),
    Revoke {
        device_id: String,
    },
    /// Replace this device's age and signing keys; peers must revoke and re-trust.
    Rotate {
        #[arg(long)]
        confirm: bool,
    },
    /// Replace sync identity, trust, clocks and tombstones, not Profile preferences.
    Reset {
        #[arg(long)]
        confirm: bool,
    },
    /// Write a new encrypted bundle for one recipient or a passphrase.
    Export(ExportArgs),
    /// Decrypt and compare a bundle; optionally write an exact apply template.
    Preview(PreviewArgs),
    /// Apply a previously prepared template with an immutable operation id.
    Apply(ApplyArgs),
    /// Settle a committed pending receipt or abort only after proving no commit.
    Abort {
        operation_id: String,
    },
}

impl Command {
    pub fn operation_id(&self) -> Option<&String> {
        match self {
            Self::Apply(args) => Some(&args.operation_id),
            Self::Abort { operation_id } => Some(operation_id),
            _ => None,
        }
    }
}

#[derive(Args)]
pub struct TrustArgs {
    #[arg(long)]
    invitation: PathBuf,
    #[arg(long)]
    expected_owner_fingerprint: String,
    #[arg(long)]
    expected_invitation_fingerprint: String,
}

#[derive(Args)]
pub struct ExportArgs {
    #[arg(long)]
    out: PathBuf,
    #[arg(long, conflicts_with = "passphrase_file")]
    recipient: Option<String>,
    #[arg(long, conflicts_with = "recipient")]
    passphrase_file: Option<PathBuf>,
    /// Export only these keys; repeat the flag. Omit for every tracked key.
    #[arg(long = "key")]
    keys: Vec<String>,
}

#[derive(Args)]
pub struct PreviewArgs {
    pub file: PathBuf,
    #[arg(long)]
    passphrase_file: Option<PathBuf>,
    /// Required for `sync apply`; preserves the exact preview for retries.
    #[arg(long)]
    out: Option<PathBuf>,
    /// Select only these keys; repeat the flag. Conflicts still need a resolution.
    #[arg(long = "key")]
    keys: Vec<String>,
    /// Accept remote content for every selected conflict.
    #[arg(long)]
    accept_conflicts: bool,
    /// Exact per-key accept, keep_local or manual merge resolutions.
    #[arg(long)]
    resolutions: Option<PathBuf>,
}

#[derive(Args)]
pub struct ApplyArgs {
    pub prepared: PathBuf,
    #[arg(long)]
    operation_id: String,
    #[arg(long)]
    passphrase_file: Option<PathBuf>,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct InvitationFile {
    format: String,
    version: u32,
    invitation: PersonalSyncInvitation,
    invitation_fingerprint: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Prepared {
    format: String,
    version: u32,
    scope: PersonalSyncProfileScope,
    envelope: PersonalSyncEnvelope,
    local_versions: Vec<PersonalSyncLocalVersion>,
    sender_device_id: String,
    device_id: String,
    expected_commit: i64,
    selections: Vec<PersonalSyncSelection>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ResolutionFile {
    version: u32,
    items: Vec<ResolutionItem>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ResolutionItem {
    key: String,
    resolution: PersonalSyncResolution,
}

pub async fn run(location: &Location, command: Command) -> Result<Value> {
    match command {
        Command::Status => control(location, "profile.sync.status", json!({})).await,
        Command::Init { out } => {
            if let Some(path) = &out {
                ensure_absent(path)?;
            }
            let invitation: PersonalSyncInvitation =
                serde_json::from_value(write(location, "profile.sync.init", json!({})).await?)
                    .map_err(|_| Error::InvalidRequest)?;
            invitation_output(&invitation, out.as_deref())
        }
        Command::Invitation { out } => {
            let invitation: PersonalSyncInvitation = serde_json::from_value(
                control(location, "profile.sync.invitation", json!({})).await?,
            )
            .map_err(|_| Error::InvalidRequest)?;
            invitation_output(&invitation, out.as_deref())
        }
        Command::Pair(args) => trust(location, "profile.sync.pair", args).await,
        Command::Trust(args) => trust(location, "profile.sync.trust", args).await,
        Command::Revoke { device_id } => {
            write(
                location,
                "profile.sync.revoke",
                json!({"device_id":device_id}),
            )
            .await
        }
        Command::Rotate { confirm } => {
            if !confirm {
                return Err(Error::InvalidRequest);
            }
            let invitation: PersonalSyncInvitation =
                serde_json::from_value(write(location, "profile.sync.rotate", json!({})).await?)
                    .map_err(|_| Error::InvalidRequest)?;
            invitation_output(&invitation, None)
        }
        Command::Reset { confirm } => {
            if !confirm {
                return Err(Error::InvalidRequest);
            }
            write(location, "profile.sync.reset", json!({"confirm":true})).await
        }
        Command::Export(args) => export(location, args).await,
        Command::Preview(args) => preview(location, args).await,
        Command::Apply(args) => apply(location, args).await,
        Command::Abort { operation_id } => {
            valid_operation_id(&operation_id)?;
            write(
                location,
                "profile.sync.abort",
                json!({"operation_id":operation_id}),
            )
            .await
        }
    }
}

async fn control(location: &Location, method: &str, params: Value) -> Result<Value> {
    precept_ipc::operator::control(location, method, params).await
}

async fn current_scope(location: &Location) -> Result<PersonalSyncProfileScope> {
    let info = control(location, "profile.info", json!({})).await?;
    let binding = info.get("binding").ok_or(Error::Unauthorized)?;
    serde_json::from_value(serde_json::json!({
        "profile_id":binding.get("profile_id").ok_or(Error::Unauthorized)?,
        "generation":binding.get("generation").ok_or(Error::Unauthorized)?,
    }))
    .map_err(|_| Error::InvalidRequest)
}

async fn write(location: &Location, method: &str, params: Value) -> Result<Value> {
    let scope = current_scope(location).await?;
    write_scoped(location, method, params, scope).await
}

async fn write_scoped(
    location: &Location,
    method: &str,
    mut params: Value,
    scope: PersonalSyncProfileScope,
) -> Result<Value> {
    let Some(fields) = params.as_object_mut() else {
        return Err(Error::InvalidRequest);
    };
    fields.insert(
        "expected_binding".into(),
        serde_json::to_value(scope).map_err(|_| Error::InvalidRequest)?,
    );
    precept_ipc::operator::project_write(location, method, params).await
}

fn invitation_output(invitation: &PersonalSyncInvitation, out: Option<&Path>) -> Result<Value> {
    let fingerprint =
        precept_sync::invitation_fingerprint(invitation).map_err(|_| Error::InvalidRequest)?;
    let file = InvitationFile {
        format: "precept-personal-sync-invitation".into(),
        version: 1,
        invitation: invitation.clone(),
        invitation_fingerprint: fingerprint.clone(),
    };
    let value = serde_json::to_value(&file).map_err(|_| Error::InvalidRequest)?;
    if let Some(path) = out {
        write_new(path, &value)?;
    }
    Ok(value)
}

async fn trust(location: &Location, method: &str, args: TrustArgs) -> Result<Value> {
    let file: InvitationFile = read(&args.invitation)?;
    if file.format != "precept-personal-sync-invitation"
        || file.version != 1
        || file.invitation_fingerprint
            != precept_sync::invitation_fingerprint(&file.invitation)
                .map_err(|_| Error::InvalidRequest)?
        || file.invitation_fingerprint != args.expected_invitation_fingerprint
        || file.invitation.owner_fingerprint != args.expected_owner_fingerprint
    {
        return Err(Error::InvalidRequest);
    }
    write(
        location,
        method,
        json!({
            "invitation":file.invitation,
            "expected_owner_fingerprint":args.expected_owner_fingerprint,
            "expected_invitation_fingerprint":args.expected_invitation_fingerprint,
        }),
    )
    .await
}

async fn export(location: &Location, args: ExportArgs) -> Result<Value> {
    ensure_absent(&args.out)?;
    if args.recipient.is_none() && args.passphrase_file.is_none() {
        return Err(Error::InvalidRequest);
    }
    let passphrase = match &args.passphrase_file {
        Some(path) => Some(read_passphrase(path)?),
        None => None,
    };
    let envelope: PersonalSyncEnvelope = serde_json::from_value(
        write(
            location,
            "profile.sync.export",
            json!({
                "recipient":args.recipient,
                "passphrase":passphrase,
                "keys":if args.keys.is_empty() { None } else { Some(args.keys) },
            }),
        )
        .await?,
    )
    .map_err(|_| Error::InvalidRequest)?;
    let value = serde_json::to_value(&envelope).map_err(|_| Error::InvalidRequest)?;
    write_new(&args.out, &value)?;
    Ok(json!({
        "written":args.out,
        "bundle_id":envelope.header.bundle_id,
        "sender_device_id":envelope.header.sender_device_id,
    }))
}

async fn preview(location: &Location, args: PreviewArgs) -> Result<Value> {
    if let Some(path) = &args.out {
        ensure_absent(path)?;
    }
    let envelope: PersonalSyncEnvelope = read(&args.file)?;
    let passphrase = match &args.passphrase_file {
        Some(path) => Some(read_passphrase(path)?),
        None => None,
    };
    let scope = current_scope(location).await?;
    let mut preview: PersonalSyncPreview = serde_json::from_value(
        control(
            location,
            "profile.sync.preview",
            json!({
                "envelope":envelope,
                "passphrase":passphrase,
                "expected_binding":scope,
            }),
        )
        .await?,
    )
    .map_err(|_| Error::InvalidRequest)?;
    let resolutions = match &args.resolutions {
        Some(path) => Some(read_resolutions(path)?),
        None => None,
    };
    select(
        &mut preview,
        &args.keys,
        args.accept_conflicts,
        resolutions.as_ref(),
    )?;
    if args.out.is_some() && preview.selections.is_empty() {
        return Err(Error::InvalidRequest);
    }
    let mut value = serde_json::to_value(&preview).map_err(|_| Error::InvalidRequest)?;
    if let Some(out) = args.out {
        let prepared = Prepared {
            format: PREPARED_FORMAT.into(),
            version: PREPARED_VERSION,
            scope,
            envelope,
            local_versions: preview.local_versions,
            sender_device_id: preview.sender_device_id,
            device_id: preview.device_id,
            expected_commit: preview.expected_commit,
            selections: preview.selections,
        };
        write_new(
            &out,
            &serde_json::to_value(&prepared).map_err(|_| Error::InvalidRequest)?,
        )?;
        value["prepared_file"] = json!(out);
    }
    Ok(value)
}

async fn apply(location: &Location, args: ApplyArgs) -> Result<Value> {
    let prepared: Prepared = read(&args.prepared)?;
    if prepared.format != PREPARED_FORMAT
        || prepared.version != PREPARED_VERSION
        || !prepared.scope.valid()
        || !prepared.envelope.valid()
        || prepared.expected_commit < 0
        || prepared.selections.is_empty()
        || !prepared
            .local_versions
            .iter()
            .all(PersonalSyncLocalVersion::valid)
        || !prepared.selections.iter().all(PersonalSyncSelection::valid)
    {
        return Err(Error::InvalidRequest);
    }
    valid_operation_id(&args.operation_id)?;
    let passphrase = match &args.passphrase_file {
        Some(path) => Some(read_passphrase(path)?),
        None => None,
    };
    let submission = PersonalSyncApplySubmission {
        operation_id: args.operation_id.clone(),
        expected_commit: prepared.expected_commit,
        channel: precept_ipc::operator::Channel::Cli,
        envelope: prepared.envelope,
        passphrase,
        local_versions: prepared.local_versions,
        sender_device_id: prepared.sender_device_id,
        device_id: prepared.device_id,
        selections: prepared.selections,
    };
    write_scoped(
        location,
        "profile.sync.apply",
        serde_json::to_value(submission).map_err(|_| Error::InvalidRequest)?,
        prepared.scope,
    )
    .await
}

fn select(
    preview: &mut PersonalSyncPreview,
    keys: &[String],
    accept_conflicts: bool,
    resolutions: Option<&BTreeMap<String, PersonalSyncResolution>>,
) -> Result<()> {
    let mut selected = BTreeSet::new();
    if !keys.is_empty()
        && keys.iter().any(|key| {
            !preview.items.iter().any(|item| item.key == *key) || !selected.insert(key.as_str())
        })
    {
        return Err(Error::InvalidRequest);
    }
    if let Some(resolutions) = resolutions {
        for key in resolutions.keys() {
            if !preview.items.iter().any(|item| item.key == *key) {
                return Err(Error::InvalidRequest);
            }
        }
    }
    let mut selections = Vec::new();
    for (mut selection, item) in preview.selections.drain(..).zip(&mut preview.items) {
        let included = keys.is_empty() || selected.contains(item.key.as_str());
        let resolved =
            resolutions.is_some_and(|map| map.contains_key(&item.key)) || accept_conflicts;
        item.selected = included
            && !item.applied
            && (item.merge != precept_ipc::operator::PersonalSyncMerge::Conflict || resolved);
        if !item.selected {
            continue;
        }
        if let Some(resolution) = resolutions.and_then(|map| map.get(&item.key)) {
            selection.resolution = resolution.clone();
        } else if accept_conflicts
            && item.merge == precept_ipc::operator::PersonalSyncMerge::Conflict
        {
            selection.resolution = PersonalSyncResolution::Accept;
        }
        selections.push(selection);
    }
    preview.selections = selections;
    Ok(())
}

fn read_resolutions(path: &Path) -> Result<BTreeMap<String, PersonalSyncResolution>> {
    let file: ResolutionFile = read(path)?;
    if file.version != 1 || file.items.len() > precept_ipc::operator::MAX_SYNC_ITEMS {
        return Err(Error::InvalidRequest);
    }
    let mut resolutions = BTreeMap::new();
    for item in file.items {
        if !(PersonalSyncSelection {
            key: item.key.clone(),
            incoming_digest: "0".repeat(64),
            expected_local_digest: "0".repeat(64),
            resolution: item.resolution.clone(),
        })
        .valid()
        {
            return Err(Error::InvalidRequest);
        }
        if resolutions.insert(item.key, item.resolution).is_some() {
            return Err(Error::InvalidRequest);
        }
    }
    Ok(resolutions)
}

fn valid_operation_id(value: &str) -> Result<()> {
    if value.is_empty()
        || value.len() > 64
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"_-".contains(&byte))
    {
        return Err(Error::InvalidRequest);
    }
    Ok(())
}

fn read_passphrase(path: &Path) -> Result<String> {
    let mut file = open_private(path)?;
    if file.metadata().map_err(|_| Error::Unavailable)?.len() > MAX_PASSPHRASE_FILE {
        return Err(Error::TooLarge);
    }
    let mut bytes = Vec::new();
    (&mut file)
        .take(MAX_PASSPHRASE_FILE + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| Error::InvalidRequest)?;
    if bytes.len() as u64 > MAX_PASSPHRASE_FILE {
        return Err(Error::TooLarge);
    }
    let mut text = String::from_utf8(bytes).map_err(|_| Error::InvalidRequest)?;
    if text.ends_with("\r\n") {
        text.truncate(text.len() - 2);
    } else if text.ends_with('\n') {
        text.pop();
    }
    Ok(text)
}

fn read<T: for<'de> Deserialize<'de>>(path: &Path) -> Result<T> {
    let mut file = open_private(path)?;
    if file.metadata().map_err(|_| Error::Unavailable)?.len() > MAX_FILE {
        return Err(Error::TooLarge);
    }
    let mut bytes = Vec::new();
    (&mut file)
        .take(MAX_FILE + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| Error::InvalidRequest)?;
    if bytes.len() as u64 > MAX_FILE {
        return Err(Error::TooLarge);
    }
    serde_json::from_slice(&bytes).map_err(|_| Error::InvalidRequest)
}

fn open_private(path: &Path) -> Result<std::fs::File> {
    match std::fs::symlink_metadata(path) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Err(Error::NotFound),
        Err(_) => Err(Error::Unavailable),
        Ok(metadata) if metadata.is_file() => precept_ipc::transport::open_private(path, false),
        Ok(_) => Err(Error::InvalidRequest),
    }
}

fn ensure_absent(path: &Path) -> Result<()> {
    match std::fs::symlink_metadata(path) {
        Ok(_) => Err(Error::Conflict),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(_) => Err(Error::Unavailable),
    }
}

fn write_new(path: &Path, value: &Value) -> Result<()> {
    ensure_absent(path)?;
    let mut file = precept_ipc::transport::new_private(path)?;
    file.write_all(&serde_json::to_vec_pretty(value).map_err(|_| Error::InvalidRequest)?)
        .map_err(|_| Error::Unavailable)?;
    file.sync_all().map_err(|_| Error::Unavailable)
}

#[cfg(test)]
mod tests;

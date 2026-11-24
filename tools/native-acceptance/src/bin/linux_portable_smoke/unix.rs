//! Unix body of the Linux packaged smoke (see the bin root).
use super::desktop;
use precept_ipc::{Access, Client, Error, Location, load_credential, transport};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::{BufRead, BufReader, Read, Write},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::mpsc,
    thread,
    time::{Duration, Instant},
};
mod archive;
mod fixture;
mod package;
mod removal;
mod scenario;

#[tokio::main]
pub async fn main() {
    scenario::main().await;
}

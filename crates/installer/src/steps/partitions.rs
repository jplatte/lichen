// SPDX-FileCopyrightText: Copyright © 2024 Serpent OS Developers
//
// SPDX-License-Identifier: MPL-2.0

//! Partititon formatting

use std::path::PathBuf;

use fs_err::tokio as fs;
use system::disk::Partition;
use tokio::process::Command;

use super::Context;

/// Format a partition
#[derive(Debug)]
pub struct FormatPartition<'a> {
    /// What partition are we formatting
    pub(crate) partition: &'a Partition,

    /// What filesystem would you like it to have
    pub(crate) filesystem: String,
}

impl<'a> FormatPartition<'a> {
    pub(super) async fn execute(&self, context: &impl Context<'a>) -> Result<(), super::Error> {
        let fs = self.filesystem.to_lowercase();
        let (exec, args) = match fs.as_str() {
            "ext4" => ("mkfs.ext4", [&self.partition.path.display().to_string()]),
            _ => unimplemented!(),
        };
        log::info!("Formatting {} as {}", self.partition.path.display(), self.filesystem);
        log::trace!("Running: {exec:?} w/ {args:?}");

        // For now we drop output, but we'll wire up stdout/stderr in context
        let mut cmd = Command::new(exec);
        cmd.args(args);
        let _ = context.run_command_captured(&mut cmd, None).await?;
        Ok(())
    }

    pub(super) fn title(&self) -> String {
        "Format partition".into()
    }

    pub(super) fn describe(&self) -> String {
        // TODO: More than ext4 xD
        format!("{} as ext4", self.partition.path.display())
    }
}

/// Mount a given partition
#[derive(Debug)]
pub struct MountPartition<'a> {
    /// Which partition?
    pub(crate) partition: &'a Partition,

    /// Where are we mounting it?
    pub(crate) mountpoint: PathBuf,
}

impl<'a> MountPartition<'a> {
    pub(super) async fn execute(&self, context: &impl Context<'a>) -> Result<(), super::Error> {
        log::info!(
            "Mounting {} to {}",
            self.partition.path.display(),
            self.mountpoint.display()
        );

        // Ensure target exists
        fs::create_dir_all(&self.mountpoint).await?;
        let source = self.partition.path.to_string_lossy().to_string();
        let dest = self.mountpoint.to_string_lossy().to_string();
        let mut cmd = Command::new("mount");
        cmd.args([&source, &dest]);

        let _ = context.run_command_captured(&mut cmd, None).await?;
        Ok(())
    }

    pub(super) fn title(&self) -> String {
        "Mount filesystem".into()
    }

    pub(super) fn describe(&self) -> String {
        format!("{} as {}", self.partition.path.display(), self.mountpoint.display())
    }
}

/// Bind mount a source dir into a target dir
#[derive(Debug)]
pub struct BindMount {
    /// The source directory
    pub(crate) source: PathBuf,

    /// Destination directory
    pub(crate) dest: PathBuf,
}

impl<'a> BindMount {
    pub(super) async fn execute(&self, context: &impl Context<'a>) -> Result<(), super::Error> {
        log::info!("Bind mounting {} to {}", self.source.display(), self.dest.display());

        // Ensure target exists
        fs::create_dir_all(&self.dest).await?;
        let source = self.source.to_string_lossy().to_string();
        let dest = self.dest.to_string_lossy().to_string();
        let mut cmd = Command::new("mount");
        cmd.args(["--bind", &source, &dest]);

        let _ = context.run_command_captured(&mut cmd, None).await?;
        Ok(())
    }

    pub(super) fn title(&self) -> String {
        "Bind mount filesystem".into()
    }

    pub(super) fn describe(&self) -> String {
        format!("{} on {}", self.source.display(), self.dest.display())
    }
}

/// Unmount a given mountpoint
#[derive(Debug)]
pub struct Unmount {
    pub(crate) mountpoint: PathBuf,
}

impl<'a> Unmount {
    pub(super) fn title(&self) -> String {
        "Unmount".to_string()
    }

    pub(super) fn describe(&self) -> String {
        format!("{}", &self.mountpoint.display())
    }

    pub(super) async fn execute(&self, context: &impl Context<'a>) -> Result<(), super::Error> {
        log::info!("Unmounting {}", self.mountpoint.display());

        let dest = self.mountpoint.to_string_lossy().to_string();
        let mut cmd = Command::new("umount");
        cmd.arg(dest);

        let _ = context.run_command_captured(&mut cmd, None).await?;
        Ok(())
    }
}

/// A cleanup helper that invokes `sync`
pub struct SyncFS {}

impl<'a> SyncFS {
    pub(super) fn title(&self) -> String {
        "Sync".into()
    }

    pub(super) fn describe(&self) -> String {
        "filesystems".into()
    }

    pub(super) async fn execute(&self, context: &impl Context<'a>) -> Result<(), super::Error> {
        log::info!("Syncing filesystems");

        let mut cmd = Command::new("sync");
        let _ = context.run_command_captured(&mut cmd, None).await;
        Ok(())
    }
}

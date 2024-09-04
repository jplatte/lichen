// SPDX-FileCopyrightText: Copyright © 2024 Serpent OS Developers
//
// SPDX-License-Identifier: MPL-2.0

//! Post-installation tasks

use std::fmt::Display;

use fs_err::tokio as fs;
use system::locale::Locale;
use tokio::process::Command;

use crate::{Account, SystemPartition};

use super::{Context, Error};

/// Configure an account on the system
#[derive(Debug)]
pub struct SetPassword<'a> {
    pub(crate) account: &'a Account,
    pub(crate) password: String,
}

/// Create an account
#[derive(Debug)]
pub struct CreateAccount<'a> {
    pub(crate) account: &'a Account,
}

impl<'a> SetPassword<'a> {
    pub(super) fn title(&self) -> String {
        "Set account password".to_string()
    }

    pub(super) fn describe(&self) -> String {
        self.account.username.clone()
    }

    /// Execute to configure the account
    pub(super) async fn execute(&self, context: &'a impl Context<'a>) -> Result<(), Error> {
        let mut cmd = Command::new("chroot");
        cmd.arg(context.root().clone());
        cmd.arg("chpasswd");

        let password_text = format!("{}:{}\n", &self.account.username, self.password);
        context.run_command_captured(&mut cmd, Some(&password_text)).await?;

        Ok(())
    }
}

impl<'a> CreateAccount<'a> {
    pub(super) fn title(&self) -> String {
        "Create account".to_string()
    }

    pub(super) fn describe(&self) -> String {
        self.account.username.clone()
    }

    pub(super) async fn execute(&self, context: &'a impl Context<'a>) -> Result<(), Error> {
        let mut cmd = Command::new("chroot");
        cmd.arg(context.root().clone());
        cmd.arg("useradd");
        cmd.arg(self.account.username.clone());
        cmd.args(["-m", "-U", "-G", "audio,adm,wheel,render,kvm,input,users"]);

        if let Some(gecos) = self.account.gecos.as_ref() {
            cmd.arg("-C");
            cmd.arg(gecos.clone());
        }
        cmd.arg("-s");
        cmd.arg(self.account.shell.clone());
        context.run_command_captured(&mut cmd, None).await?;
        Ok(())
    }
}

/// Update locale in `locale.conf`
#[derive(Debug)]
pub struct SetLocale<'a> {
    pub(crate) locale: &'a Locale<'a>,
}

impl<'a> SetLocale<'a> {
    pub(super) fn title(&self) -> String {
        "Set system locale".to_string()
    }

    pub(super) fn describe(&self) -> String {
        self.locale.display_name.clone()
    }

    pub(super) async fn execute(&self, context: &'a impl Context<'a>) -> Result<(), Error> {
        let contents = format!("LANG={}\n", self.locale.name);
        let path = context.root().join("etc").join("locale.conf");
        fs::write(path, &contents).await?;

        Ok(())
    }
}

/// Set a machine ID up in the root
#[derive(Debug)]
pub struct SetMachineID {}

impl<'a> SetMachineID {
    pub(super) fn title(&self) -> String {
        "Allocate machine-id".to_string()
    }

    pub(super) fn describe(&self) -> String {
        "via systemd-machine-id-setup".to_string()
    }

    pub(super) async fn execute(&self, context: &'a impl Context<'a>) -> Result<(), Error> {
        let file = context.root().join("etc").join("machine-id");
        if file.exists() {
            fs::remove_file(file).await?;
        }

        let mut cmd = Command::new("chroot");
        cmd.arg(context.root().clone());
        cmd.arg("systemd-machine-id-setup");
        context.run_command_captured(&mut cmd, None).await?;

        Ok(())
    }
}

#[derive(Debug)]
pub enum FstabEntry {
    Comment(String),
    Device {
        fs: String,
        mountpoint: String,
        kind: String,
        opts: String,
        dump: u8,
        pass: u8,
    },
}

impl Display for FstabEntry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FstabEntry::Comment(c) => f.write_fmt(format_args!("# {c}")),
            FstabEntry::Device {
                fs,
                mountpoint,
                kind,
                opts,
                dump,
                pass,
            } => f.write_fmt(format_args!("{fs}\t{mountpoint}\t{kind}\t{opts}\t{dump}\t{pass}")),
        }
    }
}

// Emit the fstab
#[derive(Debug)]
pub struct EmitFstab {
    entries: Vec<FstabEntry>,
}

impl Default for EmitFstab {
    fn default() -> Self {
        Self {
            entries: vec![
                // template header
                FstabEntry::Comment("/etc/fstab: static filesystem information.".to_string()),
                FstabEntry::Comment(String::new()),
                FstabEntry::Comment("<fs>	<mountpoint>	<type>	<opts>	<dump>	<pass>".to_string()),
                FstabEntry::Comment(String::new()),
                FstabEntry::Comment("/dev/ROOT	/	ext3 	noatime	0	1".to_string()),
                FstabEntry::Comment("/dev/SWAP	none	swap	sw	0	0".to_string()),
                FstabEntry::Comment("/dev/fd0	/mnt/floppy	auto	noauto	0	0".to_string()),
                // proc
                FstabEntry::Device {
                    fs: "none".into(),
                    mountpoint: "/proc".into(),
                    kind: "proc".into(),
                    opts: "nosuid,noexec".into(),
                    dump: 0,
                    pass: 0,
                },
                // shm
                FstabEntry::Device {
                    fs: "none".into(),
                    mountpoint: "/dev/shm".into(),
                    kind: "tmpfs".into(),
                    opts: "defaults".into(),
                    dump: 0,
                    pass: 0,
                },
            ],
        }
    }
}

impl TryFrom<&SystemPartition> for FstabEntry {
    type Error = self::Error;
    fn try_from(value: &SystemPartition) -> Result<Self, Error> {
        // Honestly, this is a bit ext4 centric, no ssd care given
        let s = Self::Device {
            // NOTE: This is always PartUUID for us, we only do GPT.
            fs: format!("PARTUUID={}", &value.partition.uuid),
            mountpoint: value.mountpoint.clone().ok_or_else(|| Error::NoMountpoint)?,
            kind: value
                .partition
                .sb
                .as_ref()
                .map(|sb| sb.to_string())
                .ok_or_else(|| Error::UnknownFilesystem)?,
            opts: "rw,errors=remount-ro".to_string(),
            dump: 0,
            pass: 1,
        };

        Ok(s)
    }
}

impl<'a> EmitFstab {
    // Create with a bunch of entries
    pub fn with_entries(self, entries: impl IntoIterator<Item = FstabEntry>) -> Self {
        Self {
            entries: self.entries.into_iter().chain(entries).collect::<Vec<_>>(),
        }
    }

    pub(super) fn title(&self) -> String {
        "Generate fstab".into()
    }

    pub(super) fn describe(&self) -> String {
        "".into()
    }

    /// Write the filesystem table
    pub(super) async fn execute(&self, context: &'a impl Context<'a>) -> Result<(), Error> {
        let file = context.root().join("etc").join("fstab");
        let entries = self.entries.iter().map(|e| e.to_string()).collect::<Vec<_>>();
        fs::write(file, entries.join("\n")).await?;
        Ok(())
    }
}

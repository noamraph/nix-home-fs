use anyhow::Result;
use clap::Parser;
use daemonize::Daemonize;
use fuser::{
    FileAttr, FileType, Filesystem, MountOption, ReplyAttr, ReplyData, ReplyDirectory, ReplyEntry,
    Request,
};
use libc::ENOENT;
use nix::unistd::User;
use std::ffi::OsStr;
use std::os::unix::prelude::OsStrExt;
use std::path::PathBuf;
use std::time::{Duration, UNIX_EPOCH};

const TTL: Duration = Duration::from_secs(1);

fn get_dir_attr(uid: u32, gid: u32) -> FileAttr {
    FileAttr {
        ino: 1,
        size: 0,
        blocks: 0,
        atime: UNIX_EPOCH,
        mtime: UNIX_EPOCH,
        ctime: UNIX_EPOCH,
        crtime: UNIX_EPOCH,
        kind: FileType::Directory,
        perm: 0o755,
        nlink: 2,
        uid,
        gid,
        rdev: 0,
        flags: 0,
        blksize: 512,
    }
}

fn get_uid_home_dir(uid: u32) -> Option<Vec<u8>> {
    Some(
        User::from_uid(uid.into())
            .ok()??
            .dir
            .as_os_str()
            .as_bytes()
            .into(),
    )
}

fn get_store_target(uid: u32) -> Vec<u8> {
    let home = get_uid_home_dir(uid).unwrap_or("UNKNOWN_HOME".into());
    [home, b"/nix/store".to_vec()].concat()
}

fn get_store_attr(uid: u32, gid: u32) -> FileAttr {
    FileAttr {
        ino: 2,
        size: get_store_target(uid).len().try_into().unwrap(),
        blocks: 1,
        atime: UNIX_EPOCH,
        mtime: UNIX_EPOCH,
        ctime: UNIX_EPOCH,
        crtime: UNIX_EPOCH,
        kind: FileType::Symlink,
        perm: 0o777,
        nlink: 1,
        uid,
        gid,
        rdev: 0,
        flags: 0,
        blksize: 512,
    }
}

fn get_var_target(uid: u32) -> Vec<u8> {
    let home = get_uid_home_dir(uid).unwrap_or("UNKNOWN_HOME".into());
    [home, b"/nix/var".to_vec()].concat()
}

fn get_var_attr(uid: u32, gid: u32) -> FileAttr {
    FileAttr {
        ino: 3,
        size: get_var_target(uid).len().try_into().unwrap(),
        blocks: 1,
        atime: UNIX_EPOCH,
        mtime: UNIX_EPOCH,
        ctime: UNIX_EPOCH,
        crtime: UNIX_EPOCH,
        kind: FileType::Symlink,
        perm: 0o777,
        nlink: 1,
        uid,
        gid,
        rdev: 0,
        flags: 0,
        blksize: 512,
    }
}

struct NixHomeFS;

impl Filesystem for NixHomeFS {
    fn lookup(&mut self, req: &Request, parent: u64, name: &OsStr, reply: ReplyEntry) {
        match (parent, name.to_str()) {
            (1, Some("store")) => reply.entry(&TTL, &get_store_attr(req.uid(), req.gid()), 0),
            (1, Some("var")) => reply.entry(&TTL, &get_var_attr(req.uid(), req.gid()), 0),
            _ => reply.error(ENOENT),
        }
    }

    fn getattr(&mut self, req: &Request, ino: u64, reply: ReplyAttr) {
        match ino {
            1 => reply.attr(&TTL, &get_dir_attr(req.uid(), req.gid())),
            2 => reply.attr(&TTL, &get_store_attr(req.uid(), req.gid())),
            3 => reply.attr(&TTL, &get_var_attr(req.uid(), req.gid())),
            _ => reply.error(ENOENT),
        }
    }

    fn readlink(&mut self, req: &Request, ino: u64, reply: ReplyData) {
        match ino {
            2 => reply.data(get_store_target(req.uid()).as_slice()),
            3 => reply.data(get_var_target(req.uid()).as_slice()),
            _ => reply.error(ENOENT),
        }
    }

    fn readdir(
        &mut self,
        _req: &Request,
        ino: u64,
        _fh: u64,
        offset: i64,
        mut reply: ReplyDirectory,
    ) {
        if ino != 1 {
            reply.error(ENOENT);
            return;
        }

        let entries = vec![
            (1, FileType::Directory, "."),
            (1, FileType::Directory, ".."),
            (2, FileType::Symlink, "store"),
            (3, FileType::Symlink, "var"),
        ];

        for (i, entry) in entries.into_iter().enumerate().skip(offset as usize) {
            // i + 1 means the index of the next entry
            if reply.add(entry.0, (i + 1) as i64, entry.1, entry.2) {
                break;
            }
        }
        reply.ok();
    }
}

#[derive(Parser)]
#[command(author, version, author, arg_required_else_help = true)]
#[clap(verbatim_doc_comment)]
/// A very simple read-only filesystem, which has just 2 symlinks, `store` and `var`,
/// pointing to `HOME/nix/store` and `HOME/nix/var`, where each user viewing the filesystem
/// sees HOME referring to his home dir. This allows a very simple multi-user nix installation.
/// -
/// To mount, run:
///   sudo nix-home-fs /nix
/// Or, add this line to /etc/fstab:
///   dummy-src /nix fuse./path/to/nix-home-fs dummy-opts 0 0
/// And run:
///   sudo mount /nix
struct Cli {
    /// Mount options. Currently only for compatibility with `mount -t fuse.<path>`
    #[arg(short, value_name = "OPTS")]
    opts: Option<String>,

    /// Run in foreground
    #[arg(long)]
    foreground: bool,

    /// If only one parameter is given, the mountpoint. If two parameters are given, ignored, for compatibility with `mount -t fuse.<path>`
    dev_or_mountpoint: PathBuf,

    /// If given, where to mount the filesystem
    mountpoint: Option<PathBuf>,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    env_logger::init();
    let mountpoint = cli.mountpoint.unwrap_or(cli.dev_or_mountpoint);
    let options = vec![
        MountOption::RO,
        MountOption::FSName("nix-home-fs".into()),
        MountOption::AllowOther,
    ];
    let mount_fs = || {
        fuser::mount2(NixHomeFS, &mountpoint, &options)
    };
    if cli.foreground {
        mount_fs()?;
    } else {
        Daemonize::new().start()?;
        mount_fs()?;
    };
    Ok(())
}

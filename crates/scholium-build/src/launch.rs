//! Shared cgroup supervisor, namespace/mount builder and profile-specific limits.
use crate::{BuildError, BuildProfile, Sandbox, SandboxProcess};
use std::{ffi::OsString, path::Path};
use tokio::process::Command;

impl Sandbox {
    pub(crate) fn spawn(
        &self,
        project: &Path,
        work: &Path,
        cache: tempfile::TempDir,
        argv: &[OsString],
    ) -> Result<SandboxProcess, BuildError> {
        let scope = format!(
            "scholium-{}-{}-{}.scope",
            if self.profile == BuildProfile::TypstPreview {
                "typst"
            } else {
                "rtex"
            },
            std::process::id(),
            cache
                .path()
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
        );
        let volume = std::sync::Arc::new(crate::quota::Volume::create(self.limits)?);
        crate::quota::copy_tree(work, &volume.path, volume.budget)?;
        let private_cache = volume.path.join(".font-cache");
        crate::cache::copy(cache.path(), &private_cache)?;
        use std::os::fd::AsRawFd;
        let mounted = volume.stage.path().join("mount");
        let template = self.command(
            project,
            &mounted,
            &mounted.join(".font-cache"),
            &scope,
            argv,
        );
        let mut command = Command::new(template.as_std().get_program());
        for arg in template.as_std().get_args() {
            if arg == "/usr/bin/bwrap" {
                command
                    .arg("/usr/bin/nsenter")
                    .arg("--preserve-credentials")
                    .arg(format!(
                        "--user=/proc/self/fd/{}",
                        volume.namespace.as_raw_fd()
                    ))
                    .arg(format!(
                        "--mount=/proc/self/fd/{}",
                        volume.mount_namespace.as_raw_fd()
                    ))
                    .arg("--");
            }
            command.arg(arg);
        }
        crate::quota::register(work, &volume);
        SandboxProcess::spawn(command, scope, cache, volume, work.to_owned())
    }

    pub(crate) fn command(
        &self,
        project: &Path,
        work: &Path,
        cache: &Path,
        scope: &str,
        argv: &[OsString],
    ) -> Command {
        let mut command = Command::new("/usr/bin/systemd-run");
        command.args([
            "--user",
            "--scope",
            "--quiet",
            "--expand-environment=no",
            "--unit",
            scope,
        ]);
        command.args([
            "-p",
            &format!("MemoryMax={}", self.limits.memory_bytes),
            "-p",
            "MemorySwapMax=0",
            "-p",
            &format!("TasksMax={}", self.limits.tasks),
            "-p",
            "OOMPolicy=kill",
            "--",
            "/usr/bin/bwrap",
        ]);
        command.args(self.arguments(project, work, cache, argv));
        command
    }

    fn arguments(
        &self,
        project: &Path,
        work: &Path,
        cache: &Path,
        argv: &[OsString],
    ) -> Vec<OsString> {
        let mut args: Vec<OsString> = [
            "--unshare-all",
            "--die-with-parent",
            "--new-session",
            "--cap-drop",
            "ALL",
            "--clearenv",
        ]
        .map(Into::into)
        .to_vec();
        for (key, value) in self.environment() {
            args.extend(["--setenv".into(), (*key).into(), (*value).into()]);
        }
        for dir in ["/usr", "/usr/bin", "/usr/lib", "/usr/share"] {
            args.extend(["--dir".into(), dir.into()]);
        }
        for (source, target) in [
            ("usr/bin", "/bin"),
            ("usr/lib", "/lib"),
            ("usr/lib", "/lib64"),
            ("lib", "/usr/lib64"),
        ] {
            args.extend(["--symlink".into(), source.into(), target.into()]);
        }
        for (target, source) in &self.mounts {
            args.extend(["--ro-bind".into(), source.into(), target.into()]);
        }
        args.extend(
            [
                "--proc",
                "/proc",
                "--dev",
                "/dev",
                "--size",
                if self.profile == BuildProfile::TypstPreview {
                    "67108864"
                } else {
                    "268435456"
                },
                "--tmpfs",
                "/tmp",
            ]
            .map(Into::into),
        );
        let output = if self.profile == BuildProfile::TypstPreview {
            "/out"
        } else {
            "/work"
        };
        args.extend(["--ro-bind".into(), project.into(), "/project".into()]);
        bind_output(&mut args, work, output);
        if self.profile != BuildProfile::TypstPreview {
            bind_output(&mut args, cache, "/rtex/texmf-var");
        }
        args.extend(
            [
                "--chdir",
                output,
                "/usr/bin/prlimit",
                "--fsize=67108864",
                "--nofile=128",
            ]
            .map(Into::into),
        );
        match self.profile {
            BuildProfile::TypstPreview => {
                args.extend(["--as=4294967296", "--cpu=30"].map(Into::into))
            }
            BuildProfile::FullCompilation => args.push("--cpu=60".into()),
            BuildProfile::Rtex => {}
        }
        args.push("--".into());
        args.extend_from_slice(argv);
        args
    }

    fn environment(&self) -> &'static [(&'static str, &'static str)] {
        if self.profile == BuildProfile::TypstPreview {
            &[
                ("LANG", "C.UTF-8"),
                ("HOME", "/tmp"),
                ("SCHOLIUM_TEST_FONTS", "bundled"),
            ]
        } else {
            &[
                ("PATH", "/usr/bin"),
                ("HOME", "/tmp"),
                ("LANG", "C.UTF-8"),
                ("RTEX_TEXLIVE_BIN", "/usr/bin"),
                ("RTEX_TEXDIR", "/rtex/tex"),
                ("TEXMFVAR", "/rtex/texmf-var"),
                ("TEXMFCONFIG", "/tmp/texmf-config"),
                ("TEXMFHOME", "/tmp/texmf-home"),
                ("TEXINPUTS", ".:/project:"),
                ("TEXPICTS", "/project:"),
            ]
        }
    }
}

fn bind_output(args: &mut Vec<OsString>, source: &Path, target: &str) {
    args.extend(["--bind".into(), source.into(), target.into()]);
}

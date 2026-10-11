//! Project environments (devenv, direnv, mise, nix develop, ...).
//!
//! A shell hook activates these when you `cd` into a project, but neokak
//! changes projects without a shell. So neokak runs a *provider* for the
//! project instead: a shell command that prints the project's environment
//! as `env -0` does. Providers come from the `neokak_env_providers` option
//! as `<marker>:<command>` entries; the first whose marker file is in the
//! project directory or above it wins. The environment goes to the programs
//! neokak starts: the floating terminal, floats like lazygit, and kak-lsp
//! with its language servers. Kakoune's own process environment can't change
//! once it runs, so kakscript can source `%opt{neokak_env}` (a shell script
//! that switches to the project environment) where it matters.

use std::ffi::OsString;
use std::os::unix::ffi::{OsStrExt, OsStringExt};
use std::path::Path;
use std::process::{Command, Stdio};

/// Environment variables, in a stable order.
pub type Env = Vec<(OsString, OsString)>;

/// A way to load a project's environment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Provider {
    /// A file (or directory) that marks a project using it, e.g. `.envrc`.
    pub marker: String,
    /// Shell command run in the project directory. It prints the environment
    /// NUL-separated (`env -0`), prints nothing when it doesn't apply after
    /// all (the tool isn't installed), or fails with a message on stderr.
    pub command: String,
}

impl Provider {
    /// Parses `<marker>:<command>` option entries, skipping malformed ones.
    pub fn parse_all(entries: &[String]) -> Vec<Self> {
        entries
            .iter()
            .filter_map(|e| {
                let (marker, command) = e.split_once(':')?;
                let (marker, command) = (marker.trim(), command.trim());
                (!marker.is_empty() && !command.is_empty())
                    .then(|| Self { marker: marker.into(), command: command.into() })
            })
            .collect()
    }

    /// The first provider whose marker is in `dir` or a parent directory.
    pub fn find<'a>(providers: &'a [Self], dir: &Path) -> Option<&'a Self> {
        providers.iter().find(|p| dir.ancestors().any(|d| d.join(&p.marker).exists()))
    }

    /// Runs the provider for `dir`, starting from `base`: `Ok(None)` when it
    /// printed nothing. Can take minutes the first time (Nix builds the
    /// environment), so run it off the main thread.
    pub fn load(&self, dir: &Path, base: &Env) -> Result<Option<Env>, String> {
        let out = Command::new("sh")
            .args(["-c", &self.command])
            .current_dir(dir)
            .env_clear()
            .envs(base.iter().cloned())
            .stdin(Stdio::null())
            .output()
            .map_err(|e| format!("{}: {e}", self.marker))?;
        if !out.status.success() {
            let err = String::from_utf8_lossy(&out.stderr);
            let last = err.lines().rev().find(|l| !l.trim().is_empty()).unwrap_or("failed");
            return Err(format!("{}: {}", self.marker, last.trim()));
        }
        if out.stdout.trim_ascii().is_empty() {
            return Ok(None);
        }
        Ok(Some(parse_env0(&out.stdout)))
    }
}

/// Variables that describe a process rather than an environment.
const PROCESS_VARS: [&str; 4] = ["PWD", "OLDPWD", "SHLVL", "_"];

fn is_process_var(k: &[u8]) -> bool {
    PROCESS_VARS.iter().any(|s| s.as_bytes() == k)
}

/// The variables neokak itself runs with.
pub fn current() -> Env {
    sorted(std::env::vars_os().filter(|(k, _)| !is_process_var(k.as_bytes())).collect())
}

fn sorted(mut env: Env) -> Env {
    env.sort();
    env
}

fn var<'a>(env: &'a Env, name: &str) -> Option<&'a OsString> {
    env.iter().find(|(k, _)| k == name).map(|(_, v)| v)
}

/// Whether `env` already has `dir`'s environment active (neokak was started
/// from a shell inside the project).
pub fn active_in(env: &Env, dir: &Path) -> bool {
    let under = |root: &[u8]| !root.is_empty() && dir.starts_with(Path::new(std::ffi::OsStr::from_bytes(root)));
    var(env, "DEVENV_ROOT").is_some_and(|r| under(r.as_bytes()))
        // direnv stores its directory as `-/path`.
        || var(env, "DIRENV_DIR").is_some_and(|d| under(d.as_bytes().strip_prefix(b"-").unwrap_or(d.as_bytes())))
}

/// Parses `env -0` output, minus the variables that describe the process
/// that printed it.
pub fn parse_env0(bytes: &[u8]) -> Env {
    sorted(
        bytes
            .split(|&b| b == 0)
            .filter_map(|entry| {
                let eq = entry.iter().position(|&b| b == b'=')?;
                let (k, v) = (&entry[..eq], &entry[eq + 1..]);
                (!k.is_empty() && !is_process_var(k))
                    .then(|| (OsString::from_vec(k.to_vec()), OsString::from_vec(v.to_vec())))
            })
            .collect(),
    )
}

/// A POSIX shell script that turns `from` into `to`: unsets what `to`
/// lacks and exports the rest.
pub fn script(from: &Env, to: &Env) -> String {
    let valid = |k: &OsString| {
        let k = k.as_bytes();
        !k.is_empty() && !k[0].is_ascii_digit() && k.iter().all(|b| b.is_ascii_alphanumeric() || *b == b'_')
    };
    let quote = |s: &OsString| format!("'{}'", String::from_utf8_lossy(s.as_bytes()).replace('\'', r"'\''"));
    let mut out = String::new();
    for (k, _) in from.iter().filter(|(k, _)| valid(k) && var(to, &k.to_string_lossy()).is_none()) {
        out.push_str(&format!("unset {}\n", k.to_string_lossy()));
    }
    for (k, v) in to.iter().filter(|(k, v)| valid(k) && var(from, &k.to_string_lossy()) != Some(v)) {
        out.push_str(&format!("export {}={}\n", k.to_string_lossy(), quote(v)));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn env(pairs: &[(&str, &str)]) -> Env {
        sorted(pairs.iter().map(|(k, v)| (OsString::from(k), OsString::from(v))).collect())
    }

    #[test]
    fn parses_env0_and_drops_process_variables() {
        let out = b"PATH=/nix/bin:/usr/bin\0GREET=a=b\0PWD=/x\0SHLVL=2\0_=/usr/bin/env\0MULTI=one\ntwo\0";
        assert_eq!(parse_env0(out), env(&[("GREET", "a=b"), ("MULTI", "one\ntwo"), ("PATH", "/nix/bin:/usr/bin")]));
    }

    #[test]
    fn script_switches_environments() {
        let from = env(&[("HOME", "/h"), ("OLD", "1"), ("PATH", "/usr/bin")]);
        let to = env(&[("HOME", "/h"), ("PATH", "/nix/bin:/usr/bin"), ("QUOTE", "it's"), ("BASH_FUNC_x%%", "()")]);
        assert_eq!(script(&from, &to), "unset OLD\nexport PATH='/nix/bin:/usr/bin'\nexport QUOTE='it'\\''s'\n");
        assert_eq!(script(&to, &to), "");
    }

    #[test]
    fn detects_an_already_active_environment() {
        let dir = Path::new("/home/me/proj/src");
        assert!(active_in(&env(&[("DEVENV_ROOT", "/home/me/proj")]), dir));
        assert!(active_in(&env(&[("DIRENV_DIR", "-/home/me/proj")]), dir));
        assert!(!active_in(&env(&[("DEVENV_ROOT", "/home/me/other")]), dir));
        assert!(!active_in(&env(&[]), dir));
    }

    #[test]
    fn parses_providers() {
        let entries = ["  .envrc : direnv exec . env -0".to_owned(), "no-colon".into(), ":cmd".into(), "m:".into()];
        assert_eq!(
            Provider::parse_all(&entries),
            [Provider { marker: ".envrc".into(), command: "direnv exec . env -0".into() }]
        );
    }

    #[test]
    fn finds_the_provider_up_the_tree_in_list_order() {
        let root = tempfile::tempdir().unwrap();
        let sub = root.path().join("a/b");
        std::fs::create_dir_all(&sub).unwrap();
        let providers = Provider::parse_all(&["first.nix:one".into(), "second.toml:two".into()]);
        assert_eq!(Provider::find(&providers, &sub), None);
        std::fs::write(sub.join("second.toml"), "").unwrap();
        assert_eq!(Provider::find(&providers, &sub).unwrap().command, "two");
        std::fs::write(root.path().join("first.nix"), "").unwrap();
        assert_eq!(Provider::find(&providers, &sub).unwrap().command, "one");
    }

    #[test]
    fn runs_providers() {
        let dir = tempfile::tempdir().unwrap();
        let base = env(&[("PATH", "/usr/bin:/bin")]);
        let run = |cmd: &str| Provider { marker: "m".into(), command: cmd.into() }.load(dir.path(), &base);
        let loaded = run("FOO=bar env -0").unwrap().unwrap();
        assert!(loaded.contains(&("FOO".into(), "bar".into())), "{loaded:?}");
        assert_eq!(run("true").unwrap(), None, "no output: doesn't apply");
        assert_eq!(run("echo nope >&2; exit 1").unwrap_err(), "m: nope");
    }
}

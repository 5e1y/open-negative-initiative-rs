//! A downloaded update laid in place of the running app: what can fail is done while the app can still
//! say so, then a helper waits for it to quit, keeps the old app if the new one does not run, and opens it.

use std::path::Path;
#[cfg(target_os = "macos")]
use std::path::PathBuf;
use std::process::Command;

/// Hands the install to the helper; the error says why the archive is left to open by hand.
pub fn prepare(archive: &Path) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    return mac(archive);
    #[cfg(windows)]
    return windows(archive);
    #[allow(unreachable_code)]
    Err(format!("{} installs by hand", archive.display()))
}

#[cfg(target_os = "macos")]
/// The `.app` an executable runs from, `X.app/Contents/MacOS/exe`; none for a bare binary.
pub fn bundle_of(exe: &Path) -> Option<PathBuf> {
    let app = exe.parent()?.parent()?.parent()?;
    (app.extension()? == "app").then(|| app.to_path_buf())
}

#[cfg(target_os = "macos")]
/// The team `codesign -dv` names; an ad-hoc or unsigned build names none.
pub fn team_in(codesign_says: &str) -> Option<String> {
    codesign_says.lines().find_map(|l| l.strip_prefix("TeamIdentifier=")).map(str::trim).filter(|t| !t.is_empty() && *t != "not set").map(str::to_string)
}

#[cfg(target_os = "macos")]
fn team(app: &Path) -> Option<String> {
    let out = Command::new("/usr/bin/codesign").args(["-dv", "--verbose=2"]).arg(app).output().ok()?;
    team_in(&String::from_utf8_lossy(&out.stderr))
}

#[cfg(target_os = "macos")]
/// Waits for the app's process to end, swaps the bundles and opens the new one; the old one is
/// deleted only once the new one runs, else it goes back in place and opens. `ALIVE` and `WAIT` let a check stand in.
pub const SWAP: &str = r#"while kill -0 "$1" 2>/dev/null; do sleep 0.2; done
if mv "$3" "$4"; then
  if mv "$2" "$3"; then
    ${OPEN:-open} "$3"; sleep ${WAIT:-8}
    if ${ALIVE:-pgrep -qf} "$3/Contents/MacOS/"; then rm -rf "$4"; exit 0; fi
    rm -rf "$3"
  fi
  mv "$4" "$3"
fi
${OPEN:-open} "$3""#;

#[cfg(target_os = "macos")]
/// Unpacks `archive`, holds the new app's team to the running one's, and starts the swap.
fn mac(archive: &Path) -> Result<(), String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let current = bundle_of(&exe).ok_or("this build does not run from an app")?;
    let ours = team(&current).ok_or("this build is not signed by a team")?;
    let staging = std::env::temp_dir().join(format!("oni-update-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&staging);
    std::fs::create_dir_all(&staging).map_err(|e| e.to_string())?;
    let unpacked = Command::new("/usr/bin/ditto").arg("-x").arg("-k").arg(archive).arg(&staging).status().map_err(|e| e.to_string())?;
    if !unpacked.success() {
        return Err("the archive does not unpack".into());
    }
    let new = std::fs::read_dir(&staging)
        .map_err(|e| e.to_string())?
        .flatten()
        .map(|e| e.path())
        .find(|p| p.extension().is_some_and(|x| x == "app"))
        .ok_or("the archive holds no app")?;
    let verified = Command::new("/usr/bin/codesign").args(["--verify", "--deep", "--strict"]).arg(&new).status().map_err(|e| e.to_string())?;
    if !verified.success() || team(&new).as_deref() != Some(ours.as_str()) {
        return Err("the new app is not signed by the same team".into());
    }
    let aside = staging.join("previous.app");
    Command::new("/bin/sh")
        .arg("-c")
        .arg(SWAP)
        .arg("sh")
        .arg(std::process::id().to_string())
        .arg(&new)
        .arg(&current)
        .arg(&aside)
        .spawn()
        .map_err(|e| e.to_string())?;
    Ok(())
}

/// Waits for the app to quit and starts the ONI.exe `lay` put in place; the one set aside is deleted
/// once the new one runs, else it goes back in place and starts. .NET calls read no path as a wildcard.
pub const SWAP_WINDOWS: &str = r#"param($Id, $Exe, $Wait = 8000)
$ErrorActionPreference = 'Stop'
Wait-Process -Id $Id -ErrorAction SilentlyContinue
$aside = "$Exe.previous"
try { $alive = -not [Diagnostics.Process]::Start($Exe).WaitForExit($Wait) } catch { $alive = $false }
if ($alive) {
  [IO.File]::Delete($aside)
} else {
  [IO.File]::Delete($Exe)
  [IO.File]::Move($aside, $Exe)
  [void][Diagnostics.Process]::Start($Exe)
}
[IO.File]::Delete($PSCommandPath)"#;

#[cfg(any(windows, test))]
/// Where `lay` keeps the running ONI.exe: `ONI.exe.previous`, beside it.
fn previous_of(exe: &Path) -> std::path::PathBuf {
    let mut name = exe.as_os_str().to_owned();
    name.push(".previous");
    name.into()
}

#[cfg(any(windows, test))]
/// Lays the archive's ONI.exe in place of `exe`, the running one set aside, while the app can still say
/// it failed; on a failure `exe` stays as it was. Windows renames a running image but never overwrites it.
fn lay(exe: &Path, archive: &Path) -> Result<(), String> {
    let refused = |e: std::io::Error| e.to_string();
    let mut zip = zip::ZipArchive::new(std::fs::File::open(archive).map_err(refused)?).map_err(|_| "the archive does not unpack")?;
    // Compress-Archive on Windows PowerShell writes `\` between names.
    let found = (0..zip.len()).find(|&i| zip.name_for_index(i).is_some_and(|n| n.rsplit(['/', '\\']).next() == Some("ONI.exe")));
    let mut entry = zip.by_index(found.ok_or("the archive holds no ONI.exe")?).map_err(|_| "the archive does not unpack")?;
    let mut next = exe.as_os_str().to_owned();
    next.push(".next");
    let laid = std::fs::File::create(&next)
        .and_then(|mut out| std::io::copy(&mut entry, &mut out))
        .and_then(|_| std::fs::rename(exe, previous_of(exe)))
        .and_then(|()| std::fs::rename(&next, exe).inspect_err(|_| unlay(exe)));
    if laid.is_err() {
        let _ = std::fs::remove_file(&next);
    }
    laid.map_err(refused)
}

#[cfg(any(windows, test))]
/// The running ONI.exe back in place of the one `lay` put there.
fn unlay(exe: &Path) {
    let _ = std::fs::rename(previous_of(exe), exe);
}

#[cfg(windows)]
/// The archive's signature was checked on download: the new ONI.exe is laid here, and the helper starts
/// it once the app quits. Only a build made for release lays it, as only a team-signed app does on the Mac.
fn windows(archive: &Path) -> Result<(), String> {
    use std::os::windows::process::CommandExt;
    if !crate::updates::stamped() {
        return Err("this build was not made for release".into());
    }
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    lay(&exe, archive)?;
    let script = std::env::temp_dir().join(format!("oni-update-{}.ps1", std::process::id()));
    // No console window flashes while the helper waits.
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let helper = std::fs::write(&script, SWAP_WINDOWS).and_then(|()| {
        Command::new("powershell")
            .args(["-NoProfile", "-ExecutionPolicy", "Bypass", "-WindowStyle", "Hidden", "-File"])
            .arg(&script)
            .arg("-Id")
            .arg(std::process::id().to_string())
            .arg("-Exe")
            .arg(&exe)
            .creation_flags(CREATE_NO_WINDOW)
            .spawn()
    });
    helper.map(drop).map_err(|e| {
        unlay(&exe);
        e.to_string()
    })
}

#[cfg(test)]
mod lay_tests {
    use super::*;

    /// A folder whose name a PowerShell path reads as a wildcard, holding the running ONI.exe.
    fn portable(name: &str) -> (std::path::PathBuf, std::path::PathBuf) {
        let folder = std::env::temp_dir().join(format!("oni-lay-{name}-{}", std::process::id())).join("Apps [portable]");
        let _ = std::fs::remove_dir_all(&folder);
        std::fs::create_dir_all(&folder).unwrap();
        std::fs::write(folder.join("ONI.exe"), "old").unwrap();
        (folder.clone(), folder.join("ONI.exe"))
    }

    /// An archive as the release packs it, `ONI\ONI.exe` beside a readme.
    fn archive(folder: &Path, exe_name: &str) -> std::path::PathBuf {
        use std::io::Write;
        let path = folder.parent().unwrap().join("ONI-windows.zip");
        let mut zip = zip::ZipWriter::new(std::fs::File::create(&path).unwrap());
        let stored = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
        zip.start_file("ONI\\README.txt", stored).unwrap();
        zip.write_all(b"readme").unwrap();
        zip.start_file(format!("ONI\\{exe_name}"), stored).unwrap();
        zip.write_all(b"new").unwrap();
        zip.finish().unwrap();
        path
    }

    #[test]
    fn the_new_exe_is_laid_and_the_running_one_set_aside_before_the_app_quits() {
        let (folder, exe) = portable("laid");
        lay(&exe, &archive(&folder, "ONI.exe")).unwrap();
        assert_eq!(std::fs::read_to_string(&exe).unwrap(), "new");
        assert_eq!(std::fs::read_to_string(previous_of(&exe)).unwrap(), "old");
        unlay(&exe);
        assert_eq!(std::fs::read_to_string(&exe).unwrap(), "old", "a helper that cannot start puts the old one back");
        // Twin: an archive without ONI.exe leaves the folder as it was.
        assert!(lay(&exe, &archive(&folder, "oni-other.exe")).is_err());
        assert_eq!(std::fs::read_dir(&folder).unwrap().count(), 1);
        let _ = std::fs::remove_dir_all(folder.parent().unwrap());
    }

    /// The helper run as the app runs it, in a folder named like a wildcard: system programs stand in
    /// for the builds, `cmd` staying open as a build that runs, `hostname` ending as one that does not.
    #[cfg(windows)]
    #[test]
    fn the_helper_keeps_the_new_exe_once_it_runs_and_the_old_one_otherwise() {
        let system = Path::new(&std::env::var_os("SystemRoot").unwrap()).join("System32");
        let (folder, exe) = portable("helper");
        let mut quit = Command::new("cmd").args(["/c", "exit"]).spawn().unwrap();
        let gone = quit.id().to_string();
        quit.wait().unwrap();
        let run = |new: &str, old: &str| {
            std::fs::copy(system.join(new), &exe).unwrap();
            std::fs::copy(system.join(old), previous_of(&exe)).unwrap();
            let script = folder.join("helper.ps1");
            std::fs::write(&script, SWAP_WINDOWS).unwrap();
            let ran = Command::new("powershell")
                .args(["-NoProfile", "-ExecutionPolicy", "Bypass", "-File"])
                .arg(&script)
                .args(["-Id", &gone, "-Exe"])
                .arg(&exe)
                .args(["-Wait", "3000"])
                .status()
                .unwrap();
            assert!(ran.success() && !script.exists(), "the helper ends and deletes itself");
        };
        let bytes = |p: &Path| std::fs::read(p).unwrap();
        run("cmd.exe", "whoami.exe");
        // The system names the image in its long spelling, which the temporary folder's may not be.
        let stop = format!("Get-Process -Name ONI | Where-Object {{ $_.Path -like '*oni-lay-helper-{}*' }} | Stop-Process -Force", std::process::id());
        let _ = Command::new("powershell").args(["-NoProfile", "-Command", &stop]).status();
        assert_eq!(bytes(&exe), bytes(&system.join("cmd.exe")));
        assert!(!previous_of(&exe).exists(), "the old one goes once the new one runs");
        // Twin: a new build that ends at once gives the old one back.
        run("hostname.exe", "whoami.exe");
        assert_eq!(bytes(&exe), bytes(&system.join("whoami.exe")));
        assert!(!previous_of(&exe).exists());
        let _ = std::fs::remove_dir_all(folder.parent().unwrap());
    }

    #[cfg(unix)]
    #[test]
    fn a_folder_the_app_cannot_write_refuses_the_update_while_the_app_runs() {
        use std::os::unix::fs::PermissionsExt;
        let (folder, exe) = portable("closed");
        let zip = archive(&folder, "ONI.exe");
        std::fs::set_permissions(&folder, std::fs::Permissions::from_mode(0o555)).unwrap();
        let refused = lay(&exe, &zip);
        std::fs::set_permissions(&folder, std::fs::Permissions::from_mode(0o755)).unwrap();
        assert!(refused.is_err());
        assert_eq!(std::fs::read_to_string(&exe).unwrap(), "old");
        assert_eq!(std::fs::read_dir(&folder).unwrap().count(), 1);
        // Twin: opened again, the same folder takes the update.
        lay(&exe, &zip).unwrap();
        assert_eq!(std::fs::read_to_string(&exe).unwrap(), "new");
        let _ = std::fs::remove_dir_all(folder.parent().unwrap());
    }
}

#[cfg(all(test, target_os = "macos"))]
mod tests {
    use super::*;

    #[test]
    fn an_app_s_executable_names_its_bundle_and_a_bare_binary_none() {
        let exe = Path::new("/Applications/Open Negative Initiative.app/Contents/MacOS/ONI");
        assert_eq!(bundle_of(exe), Some(PathBuf::from("/Applications/Open Negative Initiative.app")));
        // Twin: a build run from the target folder installs nothing.
        assert_eq!(bundle_of(Path::new("/x/target/release/oni")), None);
    }

    #[test]
    fn a_team_signed_build_names_its_team_and_an_ad_hoc_one_none() {
        assert_eq!(team_in("Identifier=fr.x\nTeamIdentifier=LQ34G3KW4W\n").as_deref(), Some("LQ34G3KW4W"));
        assert_eq!(team_in("Identifier=fr.x\nTeamIdentifier=not set\n"), None);
    }

    #[test]
    fn the_swap_keeps_the_new_app_once_it_runs_and_the_old_one_otherwise() {
        let root = std::env::temp_dir().join(format!("oni-swap-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("old.app")).unwrap();
        std::fs::create_dir_all(root.join("new.app")).unwrap();
        std::fs::write(root.join("new.app/mark"), "new").unwrap();
        // A check never launches an app: `OPEN` does nothing, `ALIVE` says whether the new one runs.
        let run = |new: &str, alive: &str| {
            Command::new("/bin/sh")
                .env("OPEN", "true")
                .env("WAIT", "0")
                .env("ALIVE", alive)
                .arg("-c")
                .arg(SWAP)
                .arg("sh")
                .arg("999999")
                .arg(root.join(new))
                .arg(root.join("old.app"))
                .arg(root.join("aside.app"))
                .status()
                .unwrap()
        };
        run("new.app", "true");
        assert_eq!(std::fs::read_to_string(root.join("old.app/mark")).unwrap(), "new");
        assert!(!root.join("aside.app").exists(), "the old app goes once the new one runs");
        // Twin: a new app that does not run leaves the old one in place.
        std::fs::write(root.join("old.app/mark"), "kept").unwrap();
        std::fs::create_dir_all(root.join("dead.app")).unwrap();
        run("dead.app", "false");
        assert_eq!(std::fs::read_to_string(root.join("old.app/mark")).unwrap(), "kept");
        // Twin: a new app that is not there leaves the old one in place.
        run("missing.app", "true");
        assert_eq!(std::fs::read_to_string(root.join("old.app/mark")).unwrap(), "kept");
        let _ = std::fs::remove_dir_all(&root);
    }
}

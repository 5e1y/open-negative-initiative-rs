//! A downloaded update laid in place of the running app, by a helper that waits for this process to
//! quit, swaps the new app in, keeps the old one if the swap fails, and opens the result.

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

/// Waits for the app to quit, unpacks the archive, swaps ONI.exe and starts it; the old one is
/// deleted only once the new one runs, else it goes back in place and starts.
pub const SWAP_WINDOWS: &str = r#"param($Id, $Zip, $Exe)
Wait-Process -Id $Id -ErrorAction SilentlyContinue
$dir = Join-Path $env:TEMP ("oni-update-" + $Id)
Remove-Item $dir -Recurse -Force -ErrorAction SilentlyContinue
Expand-Archive -Path $Zip -DestinationPath $dir -Force
$new = Get-ChildItem $dir -Recurse -Filter ONI.exe | Select-Object -First 1
if ($new) {
  $aside = "$Exe.previous"
  Remove-Item $aside -Force -ErrorAction SilentlyContinue
  Move-Item $Exe $aside
  try {
    Copy-Item $new.FullName $Exe -ErrorAction Stop
    Start-Process $Exe
    Start-Sleep -Seconds 8
    if (Get-Process -Name ONI -ErrorAction SilentlyContinue | Where-Object { $_.Path -eq $Exe }) {
      Remove-Item $aside -Force -ErrorAction SilentlyContinue
      exit
    }
    Remove-Item $Exe -Force -ErrorAction SilentlyContinue
  } catch {}
  Move-Item $aside $Exe -Force
}
Start-Process $Exe"#;

#[cfg(windows)]
/// The archive's signature was checked on download: the helper script does the rest once the app quits.
fn windows(archive: &Path) -> Result<(), String> {
    use std::os::windows::process::CommandExt;
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let script = std::env::temp_dir().join(format!("oni-update-{}.ps1", std::process::id()));
    std::fs::write(&script, SWAP_WINDOWS).map_err(|e| e.to_string())?;
    // No console window flashes while the helper waits.
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    Command::new("powershell")
        .args(["-NoProfile", "-ExecutionPolicy", "Bypass", "-WindowStyle", "Hidden", "-File"])
        .arg(&script)
        .arg("-Id")
        .arg(std::process::id().to_string())
        .arg("-Zip")
        .arg(archive)
        .arg("-Exe")
        .arg(&exe)
        .creation_flags(CREATE_NO_WINDOW)
        .spawn()
        .map_err(|e| e.to_string())?;
    Ok(())
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

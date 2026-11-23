//! The Kit's vault against a counting one: a launch signed in reads nothing from it and asks the service
//! nothing; the first use reads it once, off the interface thread; a sign-out and a sign-in round trip.

use crate::common;
use crate::kit::{self, EMPTY, WAIT, calls};
use i_slint_backend_testing::AccessibleRole as Role;
use oni_app::app::{FolderVault, TldbSite, Vault};
use oni_app::{Kit, KitLink};
use oni_tldb::Kept;
use slint::{ComponentHandle, Model};
use std::sync::{Arc, Mutex};
use std::thread::ThreadId;

/// The support folder's vault, each call counted with the thread that made it.
struct Counting {
    inner: FolderVault,
    reads: Mutex<Vec<ThreadId>>,
    writes: Mutex<Vec<ThreadId>>,
    erases: Mutex<Vec<ThreadId>>,
}

impl Vault for Counting {
    fn read(&self) -> Option<String> {
        self.reads.lock().unwrap().push(std::thread::current().id());
        self.inner.read()
    }

    fn write(&self, token: &str) -> Result<(), String> {
        self.writes.lock().unwrap().push(std::thread::current().id());
        self.inner.write(token)
    }

    fn erase(&self) {
        self.erases.lock().unwrap().push(std::thread::current().id());
        self.inner.erase();
    }
}

impl Counting {
    fn reads(&self) -> Vec<ThreadId> {
        self.reads.lock().unwrap().clone()
    }
}

/// A shell signed in by what a sign-in leaves, its vault counted.
fn staged(name: &str, fake: &oni_tldb::fake::Fake) -> Option<(kit::Stage, Arc<Counting>)> {
    let root = std::env::temp_dir().join(format!("oni-app-{name}-{}", std::process::id()));
    let vault = Arc::new(Counting { inner: FolderVault(root.join("Support")), reads: Mutex::default(), writes: Mutex::default(), erases: Mutex::default() });
    let stage = kit::stage_with(name, TldbSite::Simulated(fake.site.clone()), Some(vault.clone()), kit::signed_in(fake))?;
    assert_eq!(stage.root, root, "the vault in the stage's support folder");
    Some((stage, vault))
}

pub(crate) fn the_launch_reads_no_vault_and_the_first_use_reads_it_once_off_the_interface() {
    let fake = oni_tldb::fake::start();
    let Some((stage, vault)) = staged("kit-vault", &fake) else { return };
    let (rig, w) = (&stage.rig, &stage.shell.window);
    let kit = w.global::<Kit>();
    let interface = std::thread::current().id();
    common::click(rig, w, EMPTY);
    rig.settle();
    assert_eq!(kit.get_link(), KitLink::In, "signed in by the copy's mark");
    assert!(vault.reads().is_empty(), "the launch reads no vault");
    assert!(fake.journal().is_empty(), "nor asks the service: {:?}", fake.journal());

    // The search sheet is the account's first use: one read, on the vault's thread, then the Kit read.
    common::click(rig, w, common::the(w, Role::Button, "Search Gear").middle());
    rig.until("the Kit read", WAIT, || kit.get_bodies().row_count() > 0);
    common::click(rig, w, common::the(w, Role::TextInput, "Search The Lens DB").middle());
    rig.key("nikon fm2");
    rig.until("the search answered", WAIT, || !kit.get_searching() && kit.get_found().row_count() > 0);
    let reads = vault.reads();
    assert_eq!(reads.len(), 1, "one read for the Kit and the search");
    assert_ne!(reads[0], interface, "the read off the interface thread");
    rig.key(&slint::SharedString::from(slint::platform::Key::Escape));
    rig.settle();

    // Signed out: the token erased on the vault's thread and revoked, none read again, the mark gone.
    kit.invoke_disconnect();
    rig.until("the token revoked", WAIT, || fake.journal().iter().any(|s| s.path == "/api/oauth/revoke"));
    assert_eq!(kit.get_link(), KitLink::Out);
    let erases = vault.erases.lock().unwrap().clone();
    assert!(erases.len() == 1 && erases[0] != interface, "erased once, off the interface: {erases:?}");
    assert!(vault.inner.read().is_none() && !Kept::load(&stage.support).signed_in(), "nothing left to sign in by");

    // Signed in again: the token written on the vault's thread and the copy marked, what the next launch reads.
    rig.settle();
    common::click(rig, w, common::the(w, Role::Button, "Connect The Lens DB").middle());
    rig.settle();
    let authorize = stage.opened.borrow().last().cloned().expect("the browser opened");
    let back = fake.approve(&authorize, false);
    std::thread::spawn(move || kit::follow(&back));
    rig.until("signed in, the Kit read", WAIT, || kit.get_link() == KitLink::In && kit.get_bodies().row_count() > 0);
    let writes = vault.writes.lock().unwrap().clone();
    assert!(writes.len() == 1 && writes[0] != interface, "written once, off the interface: {writes:?}");
    rig.until("the token kept", WAIT, || vault.inner.read().is_some());
    assert!(Kept::load(&stage.support).signed_in(), "the copy marked signed in");
    assert_eq!(vault.reads().len(), 1, "a sign-in reads nothing");
    // Twin: a read on this thread is counted and named as the interface's, so one made there would fail above.
    let _ = vault.read();
    assert_eq!(vault.reads().last(), Some(&interface));
    let _ = std::fs::remove_dir_all(&stage.root);
}

pub(crate) fn a_sign_out_never_read_reads_the_token_once_to_revoke_it() {
    let fake = oni_tldb::fake::start();
    let Some((stage, vault)) = staged("kit-vault-out", &fake) else { return };
    let (rig, w) = (&stage.rig, &stage.shell.window);
    let kit = w.global::<Kit>();
    let interface = std::thread::current().id();
    rig.settle();
    kit.invoke_disconnect();
    assert_eq!(kit.get_link(), KitLink::Out, "out at once, whatever the vault takes");
    rig.until("the token revoked", WAIT, || fake.journal().iter().any(|s| s.path == "/api/oauth/revoke"));
    let reads = vault.reads();
    assert!(reads.len() == 1 && reads[0] != interface, "read once to revoke it, off the interface: {reads:?}");
    assert!(vault.inner.read().is_none(), "erased");
    assert!(calls(&fake, "/kit").is_empty(), "twin: the Kit itself never read");
    let _ = std::fs::remove_dir_all(&stage.root);
}

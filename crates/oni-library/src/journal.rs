//! The year's gestures, kept on this machine alone for the Wrapped and never sent: fixed-size
//! records appended to one file, the photos and names they point at kept once each in a table apart.

use oni_settings::Date;
use std::collections::HashMap;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock, mpsc};

/// Appended only, so a file written before still reads every kind it holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum Kind {
    Opened = 1,
    Edited,
    Exported,
    Medal,
    Preset,
    Balanced,
    Rolled,
    /// A retired key's, kept so a file written before reads; `value` counts frames.
    Exposed,
    Pasted,
    Measured,
}

impl Kind {
    pub const ALL: [Kind; 10] = [
        Kind::Opened,
        Kind::Edited,
        Kind::Exported,
        Kind::Medal,
        Kind::Preset,
        Kind::Balanced,
        Kind::Rolled,
        Kind::Exposed,
        Kind::Pasted,
        Kind::Measured,
    ];

    pub fn from_raw(raw: u8) -> Option<Kind> {
        Self::ALL.into_iter().find(|k| *k as u8 == raw)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Record {
    pub kind: Kind,
    pub detail: u16,
    /// Seconds since `EPOCH`, which carries the file to 2156.
    pub time: u32,
    pub subject: u32,
    pub value: u32,
}

pub const NOTHING: u32 = u32::MAX;
pub const RECORD_SIZE: usize = 16;
pub const MAGIC: [u8; 4] = *b"ONIW";
pub const VERSION: u16 = 1;

/// 2020-01-02 00:00 UTC, the origin every stored timestamp counts from.
pub fn epoch() -> Date {
    Date::since_reference_date(599_616_000.0)
}

pub struct Journal {
    /// Every file touch happens on the journal's own thread, so the interface thread only hands a
    /// gesture over, as Swift's utility queue does.
    queue: Mutex<mpsc::Sender<Job>>,
}

enum Job {
    Note { record: Record, subject: Option<String>, name: Option<String> },
    Read(mpsc::SyncSender<(Vec<Record>, Vec<String>)>),
}

/// The thread's side: the folder, the name table's indices and its line count, read once and then kept.
struct Writer {
    folder: PathBuf,
    indices: Option<(HashMap<String, u32>, u32)>,
}

impl Journal {
    /// `<support>/Journal`, one journal per folder for the process: two tables on one file would
    /// hand the same index to two names.
    pub fn in_support(support: &Path) -> Arc<Journal> {
        static SHARED: OnceLock<Mutex<HashMap<PathBuf, Arc<Journal>>>> = OnceLock::new();
        let folder = support.join("Journal");
        let Ok(mut shared) = SHARED.get_or_init(Default::default).lock() else { return Arc::new(Journal::new(folder)) };
        shared.entry(folder.clone()).or_insert_with(|| Arc::new(Journal::new(folder))).clone()
    }

    pub fn new(folder: PathBuf) -> Journal {
        let (tx, rx) = mpsc::channel::<Job>();
        let mut writer = Writer { folder, indices: None };
        let _ = std::thread::Builder::new().name("journal".into()).spawn(move || {
            for job in rx {
                match job {
                    Job::Note { mut record, subject, name } => {
                        if let Some(subject) = subject {
                            record.subject = writer.index(&subject);
                        }
                        if let Some(name) = name {
                            record.value = writer.index(&name);
                        }
                        let _ = writer.write(&encode(&record), &writer.events(), Some(&header()));
                    }
                    Job::Read(reply) => {
                        let records = decode(&std::fs::read(writer.events()).unwrap_or_default());
                        let _ = reply.send((records, writer.lines()));
                    }
                }
            }
        });
        Journal { queue: Mutex::new(tx) }
    }

    /// Appends one gesture; `subject` and `name` enter the table the first time they are seen.
    pub fn note(&self, kind: Kind, subject: Option<&str>, detail: u16, value: u32, name: Option<&str>, at: Date) {
        let elapsed = (crate::seconds(at) - crate::seconds(epoch())).trunc();
        let seconds = if elapsed <= 0.0 {
            0
        } else if elapsed >= f64::from(u32::MAX) {
            u32::MAX
        } else {
            elapsed as u32
        };
        let record = Record { kind, detail, time: seconds, subject: NOTHING, value };
        if let Ok(queue) = self.queue.lock() {
            let _ = queue.send(Job::Note { record, subject: subject.map(str::to_owned), name: name.map(str::to_owned) });
        }
    }

    /// Every gesture handed over before, read once the thread has written them.
    pub fn read(&self) -> (Vec<Record>, Vec<String>) {
        let (tx, rx) = mpsc::sync_channel(1);
        if let Ok(queue) = self.queue.lock() {
            let _ = queue.send(Job::Read(tx));
        }
        rx.recv().unwrap_or_default()
    }
}

impl Writer {
    fn events(&self) -> PathBuf {
        self.folder.join("events.bin")
    }

    fn names(&self) -> PathBuf {
        self.folder.join("names.txt")
    }

    /// A name's line in the table, keyed as written; a new one is the file's next line, or
    /// `NOTHING` when it cannot be written, so a record never points at another name's line.
    fn index(&mut self, name: &str) -> u32 {
        if self.indices.is_none() {
            self.indices = Some(self.load_indices());
        }
        let line = name.replace('\n', " ");
        if let Some(known) = self.indices.as_ref().and_then(|(table, _)| table.get(&line)) {
            return *known;
        }
        if !self.write(format!("{line}\n").as_bytes(), &self.names(), None) {
            return NOTHING;
        }
        let (table, count) = self.indices.as_mut().expect("loaded above");
        let next = *count;
        table.insert(line, next);
        *count += 1;
        next
    }

    fn lines(&self) -> Vec<String> {
        let text = std::fs::read_to_string(self.names()).unwrap_or_default();
        let mut lines: Vec<String> = text.split('\n').map(str::to_owned).collect();
        lines.pop();
        lines
    }

    /// Each name at its first line, and the number of lines, which a duplicate line sets above the names'.
    fn load_indices(&self) -> (HashMap<String, u32>, u32) {
        let lines = self.lines();
        let mut table = HashMap::new();
        for (rank, line) in lines.iter().enumerate() {
            table.entry(line.clone()).or_insert(rank as u32);
        }
        (table, lines.len() as u32)
    }

    fn write(&self, data: &[u8], path: &Path, header: Option<&[u8]>) -> bool {
        if std::fs::create_dir_all(&self.folder).is_err() {
            return false;
        }
        if !path.exists() && std::fs::write(path, header.unwrap_or(&[])).is_err() {
            return false;
        }
        std::fs::OpenOptions::new().append(true).open(path).is_ok_and(|mut file| file.write_all(data).is_ok())
    }
}

pub fn header() -> Vec<u8> {
    let mut data = MAGIC.to_vec();
    data.extend_from_slice(&VERSION.to_le_bytes());
    data.extend_from_slice(&(RECORD_SIZE as u16).to_le_bytes());
    data
}

pub fn encode(record: &Record) -> Vec<u8> {
    let mut data = vec![record.kind as u8, 0];
    data.extend_from_slice(&record.detail.to_le_bytes());
    data.extend_from_slice(&record.time.to_le_bytes());
    data.extend_from_slice(&record.subject.to_le_bytes());
    data.extend_from_slice(&record.value.to_le_bytes());
    data
}

/// Every whole record after a header this version reads; a torn tail and an unknown kind are
/// skipped, and a header from another version reads as nothing rather than as noise.
pub fn decode(bytes: &[u8]) -> Vec<Record> {
    let head = header();
    if bytes.len() < head.len()
        || bytes[..4] != MAGIC
        || u16::from_le_bytes([bytes[4], bytes[5]]) != VERSION
        || usize::from(u16::from_le_bytes([bytes[6], bytes[7]])) != RECORD_SIZE
    {
        return Vec::new();
    }
    let u16_at = |at: usize| u16::from_le_bytes([bytes[at], bytes[at + 1]]);
    let u32_at = |at: usize| u32::from_le_bytes([bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]]);
    let mut out = Vec::new();
    let mut at = head.len();
    while at + RECORD_SIZE <= bytes.len() {
        if let Some(kind) = Kind::from_raw(bytes[at]) {
            out.push(Record {
                kind,
                detail: u16_at(at + 2),
                time: u32_at(at + 4),
                subject: u32_at(at + 8),
                value: u32_at(at + 12),
            });
        }
        at += RECORD_SIZE;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scratch::Scratch;

    #[test]
    fn three_gestures_read_back_whole_and_a_torn_or_foreign_file_does_not() {
        let scratch = Scratch::new("journal");
        let journal = Journal::new(scratch.path().to_path_buf());
        let day = Date::since_reference_date(812_000_000.0);
        journal.note(Kind::Opened, Some("/a.RW2"), 0, 0, None, day);
        journal.note(Kind::Edited, Some("/a.RW2"), 7, 0, None, crate::later(day, 60.0));
        journal.note(Kind::Preset, Some("/b.RW2"), 0, 0, Some("Portra warm"), crate::later(day, 120.0));
        let (records, names) = journal.read();
        assert_eq!(records.len(), 3);
        assert_eq!(names, ["/a.RW2", "/b.RW2", "Portra warm"]);
        assert_eq!((records[1].detail, records[2].subject, records[2].value), (7, 1, 2));
        assert_eq!(records[0].time, (crate::seconds(day) - crate::seconds(epoch())) as u32);
        let bytes = std::fs::read(scratch.path().join("events.bin")).unwrap();
        assert_eq!(bytes.len(), header().len() + 3 * RECORD_SIZE);
        // Twin: a torn tail is dropped, and another version reads as nothing.
        let mut torn = bytes.clone();
        torn.extend_from_slice(&[1, 0, 3, 0, 9, 9, 9]);
        assert_eq!(decode(&torn).len(), 3);
        let mut other = bytes.clone();
        other[4] = 99;
        assert!(decode(&other).is_empty());
        // A second journal on the same folder takes its names from the table on disk.
        let reopened = Journal::new(scratch.path().to_path_buf());
        reopened.note(Kind::Exported, Some("/b.RW2"), 0, 0, None, day);
        assert_eq!(reopened.read().0[3].subject, 1);
        assert_eq!(reopened.read().1.len(), 3);
    }

    /// A gesture handed over returns at once, its write left to the journal's thread; a read waits
    /// for every write handed before it.
    #[test]
    fn a_gesture_is_written_off_the_caller_s_thread() {
        let scratch = Scratch::new("journal-thread");
        let journal = Journal::new(scratch.path().to_path_buf());
        let day = Date::since_reference_date(812_000_000.0);
        journal.note(Kind::Opened, Some("/first.RW2"), 0, 0, None, day);
        // The thread held on a reply nobody takes yet: what is handed over now waits behind it.
        let (tx, held) = mpsc::sync_channel(0);
        journal.queue.lock().unwrap().send(Job::Read(tx)).unwrap();
        for i in 0..50 {
            journal.note(Kind::Edited, Some(&format!("/f{i}.RW2")), 1, 0, None, day);
        }
        let written = || decode(&std::fs::read(scratch.path().join("events.bin")).unwrap_or_default()).len();
        assert!(written() <= 1, "twin: the caller wrote nothing itself");
        let _ = held.recv();
        assert_eq!(journal.read().0.len(), 51, "the read sees every gesture handed before it");
        assert_eq!(written(), 51);
    }

    /// A record names the line it was written on, past a duplicate line and a name holding a newline.
    #[test]
    fn a_new_name_takes_the_file_s_next_line_past_a_duplicate() {
        let scratch = Scratch::new("journal-duplicate");
        std::fs::write(scratch.path().join("names.txt"), "/a.RW2\n/a.RW2\n").unwrap();
        let journal = Journal::new(scratch.path().to_path_buf());
        let day = Date::since_reference_date(812_000_000.0);
        journal.note(Kind::Opened, Some("/b.RW2"), 0, 0, None, day);
        journal.note(Kind::Preset, Some("/a.RW2"), 0, 0, Some("two\nlines"), day);
        journal.note(Kind::Preset, Some("/a.RW2"), 0, 0, Some("two lines"), day);
        let (records, names) = journal.read();
        assert_eq!(names, ["/a.RW2", "/a.RW2", "/b.RW2", "two lines"]);
        assert_eq!(names[records[0].subject as usize], "/b.RW2");
        assert_eq!((records[1].subject, records[1].value, records[2].value), (0, 3, 3));
        // Twin: the count of distinct names lands on the duplicate, another name's line.
        assert_ne!(names[1], "/b.RW2");
    }

    #[test]
    fn one_support_folder_holds_one_journal_for_the_process() {
        let scratch = Scratch::new("journal-shared");
        let (a, b) = (Journal::in_support(scratch.path()), Journal::in_support(scratch.path()));
        assert!(std::sync::Arc::ptr_eq(&a, &b));
        // Twin: another folder is another journal.
        assert!(!std::sync::Arc::ptr_eq(&a, &Journal::in_support(&scratch.path().join("other"))));
    }
}

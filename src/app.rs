use clap::ArgMatches;
use failure::Error;
use indicatif::*;
use std::ops::{Deref, DerefMut};
use std::path::PathBuf;
use std::sync::mpsc::*;
use std::sync::*;
use std::thread;
use std::time::*;

use avgspeed::*;
use copy::*;

pub type Result<T> = std::result::Result<T, Error>;

/// utility to track changes of variable
#[derive(Default, Clone)]
pub struct TrackChange<T: PartialEq> {
    val: T,
    changed: bool,
}

impl<T: PartialEq> TrackChange<T> {
    pub fn new(val: T) -> Self {
        TrackChange {
            val,
            changed: false,
        }
    }
    pub fn changed(&mut self) -> bool {
        let r = self.changed;
        self.changed = false;
        r
    }
    pub fn set(&mut self, val: T) {
        if val == self.val {
            return;
        }
        self.changed = true;
        self.val = val;
    }
}
impl<T: PartialEq> Deref for TrackChange<T> {
    type Target = T;
    fn deref(&self) -> &T {
        &self.val
    }
}
impl<T: PartialEq> DerefMut for TrackChange<T> {
    fn deref_mut(&mut self) -> &mut T {
        self.changed = true; // XXX not checking prev value
        &mut self.val
    }
}

pub struct OperationStats {
    files_done: u32,
    bytes_done: u64,
    files_total: TrackChange<u64>,
    bytes_total: TrackChange<u64>,
    current_total: TrackChange<u64>,
    current_done: u64,
    current_path: TrackChange<PathBuf>,
    current_start: Instant,
}

impl Default for OperationStats {
    fn default() -> Self {
        OperationStats {
            files_done: 0,
            bytes_done: 0,
            files_total: TrackChange::new(0),
            bytes_total: TrackChange::new(0),
            current_total: TrackChange::new(0),
            current_done: 0,
            current_path: TrackChange::new(PathBuf::new()),
            current_start: Instant::now(),
        }
    }
}

struct SourceWalker {}

impl SourceWalker {
    fn run(tx: Sender<(PathBuf, PathBuf, u64, std::fs::Permissions, bool)>, sources: Vec<PathBuf>) {
        thread::spawn(move || {
            for src in sources {
                // let src = PathAbs::new(&src).unwrap().as_path().to_owned();
                let src = src.canonicalize().unwrap();
                for entry in walkdir::WalkDir::new(src.clone()) {
                    match entry {
                        Ok(entry) => {
                            if entry.file_type().is_file() || entry.path_is_symlink() {
                                let m = entry.metadata().unwrap();
                                let size = m.len();
                                let perm = m.permissions();
                                let is_link = m.file_type().is_symlink();
                                tx.send((src.clone(), entry.into_path(), size, perm, is_link))
                                    .expect("send");
                            }
                        }
                        Err(_) => {
                            // TODO
                        }
                    }
                }
            }
        });
    }
}

pub struct App {
    pb_curr: ProgressBar,
    pb_files: ProgressBar,
    pb_bytes: ProgressBar,
    pb_name: ProgressBar,
    last_update: Instant,
    pb_done: Arc<Mutex<()>>,
    avg_speed: AvgSpeed,
}

impl App {
    pub fn new() -> Self {
        let pb_name = ProgressBar::with_draw_target(Some(10_u64), ProgressDrawTarget::stdout());
        // \u{00A0} (nbsp) to make indicatif draw lines as wide as possible
        // otherwise it leaves leftovers from prev lines at the end of lines
        pb_name.set_style(
            ProgressStyle::default_spinner()
                .template("{spinner} {wide_msg} \u{00A0}")
                .unwrap(),
        );
        let pb_curr = ProgressBar::new(10);
        pb_curr.set_style(ProgressStyle::default_bar()
            .template("current {bar:40.} {bytes:>8}/{total_bytes:<8} {elapsed:>5} ETA {eta} {wide_msg} \u{00A0}").unwrap()
        );
        let pb_files = ProgressBar::with_draw_target(Some(10_u64), ProgressDrawTarget::stdout());
        pb_files.set_style(
            ProgressStyle::default_bar()
                .template("files   {bar:40} {pos:>8}/{len:<8} {wide_msg} \u{00A0}")
                .unwrap(),
        );
        let pb_bytes = ProgressBar::with_draw_target(Some(10), ProgressDrawTarget::stdout());
        pb_bytes.set_style(ProgressStyle::default_bar()
            .template("bytes   {bar:40} {bytes:>8}/{total_bytes:<8} {elapsed:>5} ETA {eta} {wide_msg} \u{00A0}").unwrap()
            // .progress_chars("=> ")
        );
        let multi_pb = MultiProgress::new();
        let pb_name = multi_pb.add(pb_name);
        let pb_curr = multi_pb.add(pb_curr);
        let pb_files = multi_pb.add(pb_files);
        let pb_bytes = multi_pb.add(pb_bytes);
        multi_pb.set_move_cursor(true);
        let pb_done = Arc::new(Mutex::new(()));
        let pb_done2 = pb_done.clone();
        let h = thread::spawn(move || {
            let _locked = pb_done2.lock().unwrap();
            //multi_pb.join().expect("join");
        });
        let _ = h.join();
        multi_pb.clear().unwrap();
        App {
            pb_curr,
            pb_files,
            pb_bytes,
            pb_name,
            last_update: Instant::now(),
            pb_done,
            avg_speed: AvgSpeed::new(),
        }
    }

    // fn error_ask(&self, err: String) -> OperationControl {
    //     OperationControl::Skip // TODO
    // }

    fn update_progress(&mut self, stats: &mut OperationStats) {
        // return;
        if Instant::now().duration_since(self.last_update) < Duration::from_millis(97) {
            return;
        }
        self.last_update = Instant::now();
        self.pb_name.tick(); // spin the spinner
        if stats.current_path.changed() {
            self.pb_name
                .set_message(format!("{}", stats.current_path.display()));
            self.pb_curr.set_length(*stats.current_total as u64);
            stats.current_start = Instant::now(); // This is inaccurate. Init current_start in copy worker and send instant with path?
            self.pb_curr.reset_elapsed();
            self.pb_curr.reset_eta();
        }
        //self.pb_curr.set_draw_delta(0);
        self.pb_curr.set_position(stats.current_done as u64);
        self.avg_speed.add(stats.bytes_done);
        self.pb_curr
            .set_message(format!("{}/s", HumanBytes(self.avg_speed.get() as u64)));

        if stats.files_total.changed() {
            self.pb_files.set_length(*stats.files_total as u64);
        }
        self.pb_files.set_position(u64::from(stats.files_done));

        if stats.bytes_total.changed() {
            self.pb_bytes.set_length(*stats.bytes_total as u64);
        }
        self.pb_bytes.set_position(stats.bytes_done as u64);
    }

    pub fn run(&mut self, matches: &ArgMatches) -> Result<()> {
        // for sending errors, progress info and other events from worker to ui:
        let (worker_tx, worker_rx) = channel::<WorkerEvent>();
        // TODO for sending user input (retry/skip/abort) to worker:
        let (_user_tx, user_rx) = channel::<OperationControl>();
        // fs walker sends files to operation
        let (src_tx, src_rx) = channel();

        let operation = OperationCopy::new(&matches, user_rx, worker_tx, src_rx)?;

        let search_path = operation.search_path();
        assert!(!search_path.is_empty());
        SourceWalker::run(src_tx, search_path);

        let mut stats: OperationStats = Default::default();

        let start = Instant::now();

        while let Ok(event) = worker_rx.recv() {
            match event {
                WorkerEvent::Stat(StatsChange::FileDone) => { stats.files_done += 1 }
                WorkerEvent::Stat(StatsChange::BytesTotal(n)) => {
                    *stats.bytes_total += n;
                    *stats.files_total += 1;
                },
                WorkerEvent::Stat(StatsChange::Current(p, chunk, done, todo)) => {
                    stats.current_path.set(p);
                    stats.current_total.set(todo);
                    stats.current_done = done;
                    stats.bytes_done += u64::from(chunk);
                }
                // WorkerEvent::Status(OperationStatus::Error(err)) => {
                //     let answer = self.error_ask(err);
                //     user_tx.send(answer).expect("send");
                // },
                // _ => {},
            }
            self.update_progress(&mut stats);
        }
        self.pb_curr.finish();
        self.pb_files.finish();
        self.pb_bytes.finish();
        self.pb_name.finish();
        let ela = Instant::now().duration_since(start);
        let _locked = self.pb_done.lock().unwrap();
        println!(
            "copied {} files ({}) in {} {}/s",
            *stats.files_total,
            HumanBytes(*stats.bytes_total as u64),
            HumanDuration(ela),
            HumanBytes(get_speed(*stats.bytes_total, &ela) as u64)
        );
        Ok(())
    }
}

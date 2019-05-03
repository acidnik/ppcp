use std::path::PathBuf;
use clap::ArgMatches;
use std::sync::mpsc::{Sender, Receiver, channel};
use std::thread;
use std::fs::{*, self};
use std::io::{*, self};
use std::collections::HashSet;
use std::time::Duration;

use crate::app::Result;


#[derive(Clone, PartialEq, Debug)]
pub enum StatsChange {
    FileDone, 
    BytesTotal(u64),
    Current(PathBuf, u32, u64, u64),
}

#[derive(Clone, PartialEq, Debug)]
pub enum OperationStatus {
    // Running,
    // Error(String),
    // Done,
}

pub enum OperationControl {
    // Abort,
    // Skip,
    // Retry,
    // SkipAll,
}

#[derive(Debug)]
pub enum WorkerEvent {
    Stat(StatsChange),
    // Status(OperationStatus),
}

pub trait Operation {
    fn search_path(&self) -> Vec<PathBuf>;
}

pub struct OperationCopy {
    sources: Vec<PathBuf>,
}

impl Operation for OperationCopy {
    fn search_path(&self) -> Vec<PathBuf> {
        self.sources.clone()
    }
}

#[derive(Fail, Debug)]
pub enum OperationError {
    #[fail(display = "Arguments missing")]
    ArgumentsMissing,
    #[fail(display = "Can not copy directory {} to file {}", src, dest)]
    DirOverFile {src: String, dest: String},
}

impl OperationCopy {
    pub fn new(matches: &ArgMatches, _user_rx: Receiver<OperationControl>, worker_tx: Sender<WorkerEvent>,
                src_rx: Receiver<(PathBuf, PathBuf, u64, Permissions, bool)>) -> Result<Self> {
        let source = match matches.values_of("source") {
            Some(files) => files.map(PathBuf::from).collect(),
            None => Vec::new(),
        };
        if source.is_empty() {
            println!("{:?}", source);
            Err(OperationError::ArgumentsMissing)?;
        }
        
        let dest = match matches.value_of("dest") {
            Some(file) => PathBuf::from(file),
            None => Err(OperationError::ArgumentsMissing)?,
        };
        
        let dest_parent = dest.parent().ok_or_else(|| io::Error::new(io::ErrorKind::Other, "dest.parent?"))?.to_owned();
        if ! dest_parent.exists() {
            fs::create_dir_all(&dest_parent)?;
        }
        let (dest_is_file, dest_dir) = if !dest.exists() {
            // if dest not exists - consider it a dir
            // cp /path/to/dir . -> must create dir and set it as dest
            // cp /dir1 /dir2 /file . -> cp /dir1/* ./dir; cp /dir2/* ./dir2; cp /file ./
            (false, dest.clone())
        }
        else {
            let meta = fs::symlink_metadata(&dest)?;
            if meta.is_file() {
                // cp /path/to/file.txt ./here/file.txt: dest_dir = ./here
                (true, dest_parent)
            }
            else {
                // cp /path/to/dir ./here/foo -> copy to/dir/* ./here/foo
                (false, dest.clone())
            }
        };
        for src in source.iter() {
            let meta = fs::symlink_metadata(&src)?;
            if dest_is_file && meta.is_dir() {
                Err(OperationError::DirOverFile{src: src.display().to_string(), dest: dest.display().to_string()})?
            }
        }
        if ! dest_is_file && !dest_dir.exists() {
            fs::create_dir_all(&dest_dir)?
        }
        let dest_dir = dest_dir.canonicalize()?;

        let (q_tx, q_rx) = channel::<(PathBuf, PathBuf, u64, Permissions, bool)>(); // source_path, source_file, total, 
        let (d_tx, d_rx) = channel::<(PathBuf, u32, u64, u64)>(); // src_path, chunk, done, total
        CopyWorker::run(dest_dir, d_tx, q_rx);
        // MockCopyWorker::run(dest_dir, d_tx, q_rx);

        {
        let worker_tx = worker_tx.clone();
        thread::spawn(move || {
            for (p, chunk, done, todo) in d_rx.iter() {
                worker_tx.send(WorkerEvent::Stat(StatsChange::Current(p, chunk, done, todo))).expect("send");
                if done >= todo {
                    worker_tx.send(WorkerEvent::Stat(StatsChange::FileDone)).expect("send");
                }
            }
        });
        }
        
        thread::spawn(move || {
            // let mut question = "".to_string();
            // let mut skip_all = true;
            while let Ok((src, path, size, perm, is_link)) = src_rx.recv() {

                worker_tx.send(WorkerEvent::Stat(StatsChange::BytesTotal(size))).expect("send");

                q_tx.send((src, path, size, perm, is_link)).expect("send");
            }
        });
        Ok(OperationCopy {
            sources: source,
        })
    }
}

struct CopyWorker {
}

impl CopyWorker {
    fn run(dest: PathBuf, tx: Sender<(PathBuf, u32, u64, u64)>, rx: Receiver<(PathBuf, PathBuf, u64, Permissions, bool)>) {
        thread::spawn(move || {
            let mut mkdird = HashSet::new();
            for (src, p, sz, perm, is_link) in rx.iter() {
                let r = if src.is_file() {
                    p.file_name().unwrap().into()
                }
                else {
                    // cp /dir1 d/
                    // src = /dir1 p = /dir1/inner/inner2/f.txt
                    // dest_dir = d/dir1/inner/inner2/f.txt
                    // diff(/dir1 /dir1/inner/inner2/f.txt) = inner/inner2/f.txt
                    let p_parent : PathBuf = src.file_name().unwrap().into();
                    p_parent.join(pathdiff::diff_paths(&p, &src).unwrap())
                };
                let dest_file = dest.join(r.clone());
                let dest_dir = dest_file.parent().unwrap().to_owned();
                if ! mkdird.contains(&dest_dir) {
                    // TODO : this will make dir foo/bar/baz and then foo/bar again
                    fs::create_dir_all(&dest_dir).unwrap();
                    mkdird.insert(dest_dir.clone());
                }
                
                if is_link {
                    let link_dest = std::fs::read_link(&p).unwrap();
                    std::os::unix::fs::symlink(&link_dest, &dest_file).unwrap_or_else(|err| {
                        eprintln!("Error creating symlink: {}", err);
                        ()
                    }); // FIXME 
                    tx.send((p, sz as u32, sz, sz)).unwrap();
                    continue;
                }

                let fwh = File::create(&dest_file).unwrap();
                fwh.set_permissions(perm).unwrap_or(()); // works on unix fs only

                let mut fr = BufReader::new(File::open(&p).unwrap());
                let mut fw = BufWriter::new(fwh);
                let mut buf = vec![0; 10_000_000];
                let mut s: u64 = 0;
                loop {
                    match fr.read(&mut buf) {
                        Ok(ds) => {
                            s += ds as u64;
                            if ds == 0 {
                                break;
                            }
                            fw.write_all(&buf[..ds]).unwrap();
                            tx.send((p.clone(), ds as u32, s, sz)).unwrap();
                        }
                        Err(e) => {
                            println!("{:?}", e);
                            break;
                        }
                    }
                }
            }
        });
    }
}

struct MockCopyWorker {}

impl MockCopyWorker {
    fn run(dest: PathBuf, tx: Sender<(PathBuf, u32, u64, u64)>, rx: Receiver<(PathBuf, PathBuf, u64)>) {
        let chunk = 1_048_576;
        thread::spawn(move || {
            for (_src, p, sz) in rx.iter() {
                let mut s = 0;
                while s < sz {
                    let ds = if s + chunk > sz { sz - s } else { chunk };
                    s += ds;
                    let delay = Duration::from_micros((ds / chunk * 100_000) as u64);
                    tx.send((p.clone(), ds as u32, s, sz)).unwrap();
                    thread::sleep(delay);
                }
            }
        });
    }
}

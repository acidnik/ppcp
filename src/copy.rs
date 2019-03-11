use std::path::PathBuf;
use clap::{Arg, SubCommand, ArgMatches};
use std::sync::mpsc::{Sender, Receiver, channel};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;
use std::fs::{*, self};
use std::io::{*, self};
use std::ops::{Deref, DerefMut};
use std::collections::HashSet;

use crate::app::Result;


#[derive(Clone, PartialEq, Debug)]
pub enum StatsChange {
    FilesDone, // no usize sinct in's just +1,
    FilesTotal, // same
    BytesTotal(usize),
    Current(PathBuf, usize, usize, usize),
}

#[derive(Clone, PartialEq, Debug)]
pub enum OperationStatus {
    Running,
    Error(String),
    Done,
}

pub enum OperationControl {
    Abort,
    Skip,
    Retry,
    SkipAll,
}

#[derive(Debug)]
pub enum WorkerEvent {
    Stat(StatsChange),
    Status(OperationStatus),
}

pub trait Operation {
    fn search_path(&self) -> Vec<PathBuf>;
}

pub struct OperationCopy {
    dest_dir: PathBuf,
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
    pub fn new(matches: &ArgMatches, user_rx: Receiver<OperationControl>, worker_tx: Sender<WorkerEvent>,
                src_rx: Receiver<(PathBuf, PathBuf)>) -> Result<Self> {
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
        /*
            source: file or dir (SF, SD)
            source multiple: all files, all dirs, mixed (SAF, SAD, SAM)

            dest: exists or not exists (DE, DN)
            dest exists: dir or file (DD, DF)
	    list(itertools.product(['SF', 'SD' 'SAF', 'SAD', 'SAM'], ['DN', 'DD','DF'],))
	    [('SF', 'DN'), -- copy file to file (create dir + create file)
	    ('SF', 'DD'),  -- copy file into dir (create file)
	    ('SF', 'DF'),  -- copy file to file (overwrite)
	    ('SD', 'DN'),  -- copy dir into dir: cp /foo/bar /zzz/abc -> mkdir /zzz/abc/bar
	    ('SD', 'DD'),  -- copy dir into existing dir: cp /foo/bar /zzz/abc -> /zzz/abc/bar
	    ('SD', 'DF'),  -- ERROR
	    ('SAF', 'DN'), -- copy all files to dir
	    ('SAF', 'DD'), -- copy all files to dir (mkdir)
	    ('SAF', 'DF'), -- ERROR
	    ('SAD', 'DN'), -- copy all dirs to dir (mkdir) cp /aaa /bbb /ccc -> /ccc/aaa, /ccc/bbb
	    ('SAD', 'DD'), -- same
	    ('SAD', 'DF'), -- ERROR
	    ('SAM', 'DN'), -- copy dirs and files to new dir
	    ('SAM', 'DD'), -- copy dirs and files to dir
	    ('SAM', 'DF')] -- ERROR

        */
        // let dest = dest.canonicalize()?;
        let dest_parent = dest.parent().ok_or(io::Error::new(io::ErrorKind::Other, "dest.parent?"))?.to_owned();
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
            let meta = fs::metadata(&dest)?;
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
            let meta = fs::metadata(&src)?;
            if dest_is_file && meta.is_dir() {
                Err(OperationError::DirOverFile{src: src.display().to_string(), dest: dest.display().to_string()})?
            }
        }
        if ! dest_is_file && !dest_dir.exists() {
            fs::create_dir_all(&dest_dir)?
        }
        let dest_dir = dest_dir.canonicalize()?;

        let (q_tx, q_rx) = channel::<(PathBuf, PathBuf, usize)>(); // source_path, source_file, total
        let (d_tx, d_rx) = channel::<(PathBuf, usize, usize, usize)>(); // src_path, chunk, done, total
        let inner_worker = CopyWorker::new(dest_dir, d_tx, q_rx);

        {
        let worker_tx = worker_tx.clone();
        thread::spawn(move || {
            for (p, chunk, done, todo) in d_rx.iter() {
                worker_tx.send(WorkerEvent::Stat(StatsChange::Current(p, chunk, done, todo))).expect("send");
                if done >= todo {
                    worker_tx.send(WorkerEvent::Stat(StatsChange::FilesDone)).expect("send");
                }
            }
        });
        }
        
        {
        thread::spawn(move || {
            let mut question = "".to_string();
            let mut skip_all = true;
            while let Ok((src, path)) = src_rx.recv() {

                worker_tx.send(WorkerEvent::Stat(StatsChange::FilesTotal)).expect("send");

                let size = match fs::metadata(&path) {
                    Ok(m) => {
                        let size = m.len() as usize;
                        worker_tx.send(WorkerEvent::Stat(StatsChange::BytesTotal(size))).expect("send");
                        size
                    },
                    Err(err) => {
                        // question = format!("{:?}: {}", p, err);
                        0
                        // TODO
                        // println!("{}", question);
                    },
                };
                q_tx.send((src, path, size)).expect("send");
            }
            drop(q_tx);
        });
        }
        Ok(OperationCopy {
            dest_dir: dest,
            sources: source,
        })
    }
}

struct CopyWorker {
}

impl CopyWorker {
    fn new(dest: PathBuf, tx: Sender<(PathBuf, usize, usize, usize)>, rx: Receiver<(PathBuf, PathBuf, usize)>) -> Self {
        thread::spawn(move || {
            let mut mkdird = HashSet::new();
            // println!("dest = {:?}", dest);
            // return;
            for (src, p, sz) in rx.iter() {
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
                // println!("{:?} -> {:?} | r = {:?}", p, dest_file, r);
                // continue;
                // let mut fr = BufReader::new(File::open(&p).unwrap());
                // let mut fw = BufWriter::new(File::create(&dest_file).unwrap());
                let mut fr = File::open(&p).unwrap();
                let mut fw = BufWriter::new(File::create(&dest_file).unwrap());
                let mut buf = vec![0; 10_000_000];
                let mut s = 0;
                loop {
                    match fr.read(&mut buf) {
                        Ok(ds) => {
                            s += ds;
                            if ds == 0 {
                                break;
                            }
                            fw.write(&mut buf[..ds]).unwrap();
                            tx.send((p.clone(), ds, s, sz)).unwrap();
                        }
                        Err(e) => {
                            println!("{:?}", e);
                            break;
                        }
                    }
                }
            }
        });
        CopyWorker {}
    }
}

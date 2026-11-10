//! Owned OpenCode child. Pipes are bounded; kill/reap is serialised with background readers.
use std::{io::{self,BufRead,BufReader,Read},process::{Child,Command,Stdio},sync::{Arc,Mutex,atomic::{AtomicBool,Ordering},mpsc::{self,Receiver}},time::{Duration,Instant}};
pub(crate) fn copy_command(c:&Command)->Command {let mut out=Command::new(c.get_program());out.args(c.get_args());if let Some(dir)=c.get_current_dir(){out.current_dir(dir);}for(k,v) in c.get_envs(){if let Some(v)=v{out.env(k,v);}else{out.env_remove(k);}}out}
pub(crate) fn kill(pid:u32){#[cfg(unix)]unsafe{libc::kill(-(pid as i32),libc::SIGKILL);}}
pub(crate) struct Process {pub child:Child,pub lines:Receiver<String>,pub stderr:Arc<Mutex<String>>,pub overflow:Arc<AtomicBool>,closed:Arc<AtomicBool>,lock:Arc<Mutex<()>>,reaped:bool}
impl Process {
 pub fn spawn(c:&mut Command)->io::Result<Self>{
  #[cfg(unix)]{use std::os::unix::process::CommandExt;c.process_group(0);}
  let mut child=c.stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn()?;
  let closed=Arc::new(AtomicBool::new(false));let lock=Arc::new(Mutex::new(()));let overflow=Arc::new(AtomicBool::new(false));let pid=child.id();
  let stderr=Arc::new(Mutex::new(String::new()));let tail=stderr.clone();let mut pipe=child.stderr.take().unwrap();
  std::thread::spawn(move||{let mut b=[0u8;8192];while let Ok(n)=pipe.read(&mut b){if n==0{break;}let mut s=tail.lock().unwrap();s.push_str(&String::from_utf8_lossy(&b[..n]));*s=super::events::text(&s,16384);}});
  let (tx,lines)=mpsc::sync_channel(256);let out=child.stdout.take().unwrap();let done=closed.clone();let guard=lock.clone();let full=overflow.clone();
  std::thread::spawn(move||{let mut r=BufReader::new(out);loop{let mut b=vec![];match (&mut r).take(8*1024*1024+1).read_until(b'\n',&mut b){Ok(0)|Err(_)=>break,Ok(_) if b.len()>8*1024*1024=>{full.store(true,Ordering::Relaxed);break;},_=>{}}
    match tx.try_send(String::from_utf8_lossy(&b).into_owned()){Ok(())=>{},Err(mpsc::TrySendError::Full(_))=>{full.store(true,Ordering::Relaxed);break;},Err(_)=>break}}
    if full.load(Ordering::Relaxed){let _g=guard.lock().unwrap();if !done.load(Ordering::Relaxed){kill(pid);}}
  });
  Ok(Self{child,lines,stderr,overflow,closed,lock,reaped:false})
 }
 pub fn exited(&mut self)->io::Result<Option<i32>>{if self.reaped{return Ok(Some(-1));}let _g=self.lock.lock().unwrap();if let Some(status)=self.child.try_wait()?{self.closed.store(true,Ordering::Relaxed);self.reaped=true;return Ok(Some(status.code().unwrap_or(-1)));}Ok(None)}
 pub fn stop(&mut self)->bool{let _g=self.lock.lock().unwrap();self.closed.store(true,Ordering::Relaxed);if self.reaped{return true;}kill(self.child.id());let _=self.child.kill();let until=Instant::now()+Duration::from_secs(1);loop{if self.child.try_wait().ok().flatten().is_some(){self.reaped=true;return true;}if Instant::now()>=until{return false;}std::thread::sleep(Duration::from_millis(10));}}
}
impl Drop for Process{fn drop(&mut self){self.stop();if !self.reaped{let _=self.child.wait();}}}

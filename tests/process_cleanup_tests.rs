use celerterm::pty::{
    collect_descendant_pids, is_pid_alive, terminate_process_tree_graceful, terminate_process_tree_sync,
    PtySession,
};
use std::process::Command;
use std::time::{Duration, Instant};

#[test]
fn test_is_pid_alive_and_collect_descendants() {
    let mut child = Command::new("sleep")
        .arg("30")
        .spawn()
        .expect("Failed to spawn sleep");

    let pid = child.id();
    assert!(is_pid_alive(pid), "Process should be alive after spawn");

    let descendants = collect_descendant_pids(pid);
    assert!(descendants.is_empty() || !descendants.contains(&pid));

    // Terminate process tree synchronously
    terminate_process_tree_sync(pid, None);

    let mut terminated = false;
    let deadline = Instant::now() + Duration::from_secs(2);
    while Instant::now() < deadline {
        if !is_pid_alive(pid) || child.try_wait().map(|st| st.is_some()).unwrap_or(false) {
            terminated = true;
            break;
        }
        std::thread::sleep(Duration::from_millis(20));
    }

    assert!(terminated, "Process should be dead after terminate_process_tree_sync");
    let _ = child.wait();
}

#[test]
fn test_terminate_process_tree_graceful() {
    let mut child = Command::new("sleep")
        .arg("30")
        .spawn()
        .expect("Failed to spawn sleep");

    let pid = child.id();
    assert!(is_pid_alive(pid), "Process should be alive after spawn");

    // Terminate gracefully with 50ms escalation
    terminate_process_tree_graceful(pid, None, 50);

    let mut terminated = false;
    let deadline = Instant::now() + Duration::from_secs(3);
    while Instant::now() < deadline {
        if !is_pid_alive(pid) || child.try_wait().map(|st| st.is_some()).unwrap_or(false) {
            terminated = true;
            break;
        }
        std::thread::sleep(Duration::from_millis(20));
    }

    assert!(terminated, "Process should be dead after graceful termination");
    let _ = child.wait();
}

#[test]
fn test_pty_session_terminate() {
    let mut pty = PtySession::spawn(80, 24, None).expect("Pty spawn succeeds");
    let child_pid = pty.child_pid.expect("Child PID must be present");
    assert!(is_pid_alive(child_pid), "PTY child shell should be alive");

    pty.terminate();

    let mut terminated = false;
    let deadline = Instant::now() + Duration::from_secs(3);
    while Instant::now() < deadline {
        if !is_pid_alive(child_pid) {
            terminated = true;
            break;
        }
        std::thread::sleep(Duration::from_millis(20));
    }

    assert!(terminated, "PTY child shell should be terminated after pty.terminate()");
}

#[test]
fn test_nested_process_tree_termination() {
    // Spawn sh which spawns sleep
    let mut child = Command::new("sh")
        .arg("-c")
        .arg("sleep 30")
        .spawn()
        .expect("Failed to spawn sh -c sleep");

    let sh_pid = child.id();
    assert!(is_pid_alive(sh_pid));

    // Wait briefly for sh to spawn sleep
    std::thread::sleep(Duration::from_millis(100));

    let descendants = collect_descendant_pids(sh_pid);
    println!("Descendants of sh({}): {:?}", sh_pid, descendants);

    terminate_process_tree_sync(sh_pid, None);

    let mut all_dead = false;
    let deadline = Instant::now() + Duration::from_secs(3);
    while Instant::now() < deadline {
        let sh_dead = !is_pid_alive(sh_pid) || child.try_wait().map(|st| st.is_some()).unwrap_or(false);
        let descendants_dead = descendants.iter().all(|&p| !is_pid_alive(p));
        if sh_dead && descendants_dead {
            all_dead = true;
            break;
        }
        std::thread::sleep(Duration::from_millis(20));
    }

    assert!(all_dead, "Both parent sh and child sleep must be terminated");
    let _ = child.wait();
}

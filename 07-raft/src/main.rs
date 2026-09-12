use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

#[derive(Clone, Copy, PartialEq, Debug)]
enum Role {
    Follower,
    Candidate,
    Leader,
}

struct Entry {
    term: u64,   // used by the commit-term safety rule (Raft Fig. 8) and log up-to-date checks
    cmd: String, // the client command, e.g. "set x 1" or "remove x"
}

struct State {
    term: u64,
    role: Role,
    voted_for: Option<String>,
    last_heard: Instant,
    log: Vec<Entry>,                               // the replicated command log
    commit_index: usize,                           // how many entries are committed
    applied: usize,                                // how many committed entries we've applied to kv
    kv: std::collections::HashMap<String, String>, // the state machine
}

fn apply(s: &mut State) {
    while s.applied < s.commit_index {
        let cmd = s.log[s.applied].cmd.clone();
        match cmd.split_whitespace().collect::<Vec<_>>().as_slice() {
            ["set", k, rest @ ..] => {
                s.kv.insert(k.to_string(), rest.join(" "));
            }
            ["remove", k] => {
                s.kv.remove(*k);
            }
            _ => {}
        }
        s.applied += 1;
    }
}

#[allow(dead_code)] // retained for the unit tests
fn port_of(addr: &str) -> u16 {
    addr.rsplit(':')
        .next()
        .and_then(|p| p.parse().ok())
        .unwrap_or(u16::MAX)
}

fn request_vote(
    peer: &str,
    term: u64,
    me: &str,
    last_index: usize,
    last_term: u64,
) -> Option<(u64, bool)> {
    let sa: std::net::SocketAddr = peer.parse().ok()?;
    let mut conn = TcpStream::connect_timeout(&sa, Duration::from_millis(500)).ok()?;
    conn.set_read_timeout(Some(Duration::from_secs(1))).ok()?;
    conn.write_all(format!("requestvote {term} {me} {last_index} {last_term}\n").as_bytes())
        .ok()?;
    let mut reply = String::new();
    BufReader::new(&conn).read_line(&mut reply).ok()?;
    match reply.split_whitespace().collect::<Vec<_>>().as_slice() {
        ["vote", t, granted] => Some((t.parse().ok()?, *granted == "yes")),
        _ => None,
    }
}

fn serialize_log(log: &[Entry]) -> String {
    if log.is_empty() {
        return "-".to_string();
    }
    log.iter()
        .map(|e| format!("{}~{}", e.term, e.cmd.replace(' ', "\u{1f}"))) // \x1f = space placeholder
        .collect::<Vec<_>>()
        .join("|")
}

fn parse_entries(blob: &str) -> Vec<Entry> {
    if blob == "-" {
        return Vec::new();
    }
    blob.split('|')
        .filter_map(|part| {
            let (t, cmd) = part.split_once('~')?;
            Some(Entry {
                term: t.parse().ok()?,
                cmd: cmd.replace('\u{1f}', " "),
            })
        })
        .collect()
}

// AppendEntries RPC → Some((their_term, their_log_len)) or None if unreachable.
fn append_entries(
    peer: &str,
    term: u64,
    leader: &str,
    commit: usize,
    entries: &str,
) -> Option<(u64, usize)> {
    let sa: std::net::SocketAddr = peer.parse().ok()?;
    let mut conn = TcpStream::connect_timeout(&sa, Duration::from_millis(500)).ok()?;
    conn.set_read_timeout(Some(Duration::from_secs(1))).ok()?;
    conn.write_all(format!("append {term} {leader} {commit} {entries}\n").as_bytes())
        .ok()?;
    let mut reply = String::new();
    BufReader::new(&conn).read_line(&mut reply).ok()?;
    match reply.split_whitespace().collect::<Vec<_>>().as_slice() {
        ["appendack", t, len] => Some((t.parse().ok()?, len.parse().ok()?)),
        _ => None,
    }
}

/// Randomized election timeout, RE-DRAWN at every expiry (the paper's scheme):
/// repeated split votes only resolve if colliding candidates draw apart next round.
fn rand_timeout() -> Duration {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.subsec_nanos() as u64)
        .unwrap_or(0);
    Duration::from_millis(1500 + nanos % 1500)
}

fn persist(s: &State, path: &str) {
    let voted = s.voted_for.as_deref().unwrap_or("-");
    let data = format!(
        "term {}\nvoted {}\ncommit {}\nlog {}\n",
        s.term,
        voted,
        s.commit_index,
        serialize_log(&s.log)
    );
    // Write-then-rename: File::create would truncate the previous state in place,
    // so a crash mid-persist could erase term/vote/log entirely — re-enabling the
    // double-vote and lost-commit failures this function exists to prevent. And a
    // FAILED persist must not be survivable: replying after a failed persist would
    // violate persist-before-externalize, so we crash instead.
    let tmp = format!("{path}.tmp");
    let mut f = std::fs::File::create(&tmp).expect("create state file");
    f.write_all(data.as_bytes()).expect("write state");
    f.sync_all().expect("fsync state"); // durable BEFORE we return (a crash can't lose it)
    std::fs::rename(&tmp, path).expect("rename state file");
}

fn load(path: &str) -> Option<(u64, Option<String>, usize, Vec<Entry>)> {
    let data = std::fs::read_to_string(path).ok()?;
    let (mut term, mut voted, mut commit, mut log) = (0u64, None, 0usize, Vec::new());
    for line in data.lines() {
        if let Some(v) = line.strip_prefix("term ") {
            term = v.parse().unwrap_or(0);
        } else if let Some(v) = line.strip_prefix("voted ") {
            voted = (v != "-").then(|| v.to_string());
        } else if let Some(v) = line.strip_prefix("commit ") {
            commit = v.parse().unwrap_or(0);
        } else if let Some(v) = line.strip_prefix("log ") {
            log = parse_entries(v);
        }
    }
    Some((term, voted, commit, log))
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let port = args.get(1).cloned().unwrap_or_else(|| "6000".to_string());
    let peers: Vec<String> = args.iter().skip(2).cloned().collect();
    let me = format!("127.0.0.1:{port}");
    let total = peers.len() + 1;
    let majority = total / 2 + 1;
    println!("node {me} — peers {peers:?} — timeout randomized 1500–3000 ms, majority {majority}");

    let state_path = format!("raft-{port}.state");
    let (t0, v0, c0, log0) = load(&state_path).unwrap_or((0, None, 0, Vec::new()));
    let mut initial = State {
        term: t0,
        role: Role::Follower,
        voted_for: v0,
        last_heard: Instant::now(),
        log: log0,
        commit_index: c0,
        applied: 0,
        kv: std::collections::HashMap::new(),
    };
    apply(&mut initial); // rebuild kv by replaying committed entries
    if t0 > 0 || !initial.log.is_empty() {
        println!(
            "recovered: term {t0}, {} log entries, commit {c0}",
            initial.log.len()
        );
    }
    let state = Arc::new(Mutex::new(initial));

    // ---- election + heartbeat thread ----
    {
        let me = me.clone();
        let peers = peers.clone();
        let state = Arc::clone(&state);
        let state_path = state_path.clone(); // the thread gets its own copy of the path
        thread::spawn(move || {
            let mut election_timeout = rand_timeout();
            loop {
                thread::sleep(Duration::from_millis(200));

                // A leader just sends heartbeats (which reset followers' election timers).
                let (role, term) = {
                    let s = state.lock().unwrap();
                    (s.role, s.term)
                };
                if role == Role::Leader {
                    // snapshot under a SHORT lock (no RPCs while holding it)
                    let (self_len, blob, commit) = {
                        let s = state.lock().unwrap();
                        (s.log.len(), serialize_log(&s.log), s.commit_index)
                    };
                    // AppendEntries to every peer. Two rules the paper's Figure 2
                    // demands and a stale leader dies without: (a) a response
                    // carrying a HIGHER term deposes us; (b) only SAME-term acks
                    // count toward commitment — a rejecting follower's log length
                    // is not agreement.
                    let mut lengths = vec![self_len]; // self counts
                    let mut max_seen_term = term;
                    for peer in &peers {
                        if let Some((t, len)) = append_entries(peer, term, &me, commit, &blob) {
                            if t > max_seen_term {
                                max_seen_term = t;
                            }
                            if t == term {
                                lengths.push(len);
                            }
                        }
                    }
                    let mut s = state.lock().unwrap();
                    if max_seen_term > s.term {
                        s.term = max_seen_term;
                        s.role = Role::Follower;
                        s.voted_for = None;
                        persist(&s, &state_path);
                        println!("term {}: {me} → FOLLOWER (higher term in ack)", s.term);
                        continue;
                    }
                    // Re-check we are STILL the leader of the term we snapshotted:
                    // the round took wall-clock time, and a RequestVote handler may
                    // have deposed us mid-round. Committing on a stale snapshot is
                    // exactly the Figure 8 violation.
                    if s.role != Role::Leader || s.term != term {
                        continue;
                    }
                    lengths.sort_unstable_by(|a, b| b.cmp(a));
                    let agreed = if lengths.len() >= majority {
                        lengths[majority - 1]
                    } else {
                        0
                    };
                    if agreed > s.commit_index
                        && agreed > 0
                        && agreed <= s.log.len()
                        && s.log[agreed - 1].term == term
                    {
                        s.commit_index = agreed;
                    }
                    persist(&s, &state_path);
                    apply(&mut s);
                    continue;
                }

                // Follower/candidate: has the leader gone silent past our timeout?
                let timed_out = { state.lock().unwrap().last_heard.elapsed() > election_timeout };
                if !timed_out {
                    continue;
                }

                // Re-draw the timeout for the NEXT round — colliding candidates
                // must be able to draw apart, or split votes can repeat forever.
                election_timeout = rand_timeout();

                // Become a candidate for a NEW term (short lock), snapshot the term.
                let (term, last_index, last_term) = {
                    let mut s = state.lock().unwrap();
                    s.term += 1;
                    s.role = Role::Candidate;
                    s.voted_for = Some(me.clone());
                    s.last_heard = Instant::now();
                    persist(&s, &state_path);
                    println!("term {}: {me} → CANDIDATE (requesting votes)", s.term);
                    (
                        s.term,
                        s.log.len(),
                        s.log.last().map(|e| e.term).unwrap_or(0),
                    )
                };

                let total = peers.len() + 1; // including self
                let mut max_seen_term = term;
                let votes: usize = {
                    let mut count = 1; // self-vote
                    for peer in &peers {
                        if let Some((t, granted)) =
                            request_vote(peer, term, &me, last_index, last_term)
                        {
                            if t > max_seen_term {
                                max_seen_term = t;
                            }
                            if granted && t == term {
                                count += 1;
                            }
                        }
                    }
                    count
                };

                // Won? Re-lock and confirm we're STILL a candidate in the SAME term.
                let mut s = state.lock().unwrap();
                if max_seen_term > s.term {
                    s.term = max_seen_term;
                    s.role = Role::Follower;
                    s.voted_for = None;
                    persist(&s, &state_path);
                    println!("term {}: {me} → FOLLOWER (higher term in vote reply)", s.term);
                    continue;
                }
                if s.role == Role::Candidate && s.term == term && votes >= majority {
                    s.role = Role::Leader;
                    println!("term {term}: {me} → LEADER ({votes}/{total} votes)");
                }
            }
        });
    }

    let listener = TcpListener::bind(format!("127.0.0.1:{port}")).unwrap();
    for conn in listener.incoming() {
        let Ok(stream) = conn else { continue };
        let mut writer = match stream.try_clone() {
            Ok(w) => w,
            Err(_) => continue,
        };
        let _ = stream.set_read_timeout(Some(Duration::from_secs(1)));
        let mut reader = BufReader::new(stream);
        let mut line = String::new();
        if reader.read_line(&mut line).is_err() {
            continue;
        }

        match line.split_whitespace().collect::<Vec<_>>().as_slice() {
            ["requestvote", term, candidate, last_index, last_term] => {
                let cand_term: u64 = term.parse().unwrap_or(0);
                let cand_last_index: usize = last_index.parse().unwrap_or(0);
                let cand_last_term: u64 = last_term.parse().unwrap_or(0);
                let candidate = candidate.to_string();
                let (my_term, granted) = {
                    let mut s = state.lock().unwrap();
                    if cand_term > s.term {
                        s.term = cand_term;
                        s.role = Role::Follower;
                        s.voted_for = None;
                    }
                    let my_last_term = s.log.last().map(|e| e.term).unwrap_or(0);
                    let my_last_index = s.log.len();
                    let log_ok = cand_last_term > my_last_term
                        || (cand_last_term == my_last_term && cand_last_index >= my_last_index);
                    let granted = cand_term == s.term
                        && (s.voted_for.is_none()
                            || s.voted_for.as_deref() == Some(candidate.as_str()))
                        && log_ok;
                    if granted {
                        s.voted_for = Some(candidate.clone());
                        s.last_heard = Instant::now();
                    }
                    persist(&s, &state_path);
                    (s.term, granted)
                };
                let _ = writeln!(
                    writer,
                    "vote {my_term} {}",
                    if granted { "yes" } else { "no" }
                );
            }

            ["append", term, _leader, commit, entries] => {
                let ae_term: u64 = term.parse().unwrap_or(0);
                let leader_commit: usize = commit.parse().unwrap_or(0);
                let entries = *entries; // &&str → &str
                let (my_term, my_len) = {
                    let mut s = state.lock().unwrap();
                    if ae_term >= s.term {
                        if ae_term > s.term {
                            s.voted_for = None;
                        }
                        s.term = ae_term;
                        s.role = Role::Follower;
                        s.last_heard = Instant::now();
                        s.log = parse_entries(entries); // adopt the leader's log (Step 2 simplification)
                        // Monotone: commitIndex never decreases (Figure 2).
                        s.commit_index = s.commit_index.max(leader_commit.min(s.log.len()));
                        apply(&mut s);
                        persist(&s, &state_path);
                    }
                    (s.term, s.log.len())
                };
                let _ = writeln!(writer, "appendack {my_term} {my_len}");
            }
            ["set", ..] | ["remove", ..] => {
                // The log wire format uses '|' and '~' as delimiters and \u{1F} as
                // the space stand-in; a command containing them would be resplit
                // into DIFFERENT entries at each replica — divergent state machines.
                if line.contains('|') || line.contains('~') || line.contains('\u{1f}') {
                    let _ = writeln!(writer, "ERR command may not contain '|', '~', or U+001F");
                    continue;
                }
                let mut s = state.lock().unwrap();
                if s.role == Role::Leader {
                    let term = s.term; // read first (ends the borrow) — the guard Derefs the WHOLE State
                    s.log.push(Entry {
                        term,
                        cmd: line.trim().to_string(),
                    });
                    persist(&s, &state_path);
                    let _ = writeln!(writer, "OK (log index {})", s.log.len());
                } else {
                    let _ = writeln!(writer, "NOT LEADER");
                }
            }
            ["get", key] => {
                let mut s = state.lock().unwrap();
                if s.role == Role::Leader {
                    apply(&mut s);
                    let value =
                        s.kv.get(*key)
                            .cloned()
                            .unwrap_or_else(|| "(nil)".to_string());
                    let _ = writeln!(writer, "{value}");
                } else {
                    let _ = writeln!(writer, "NOT LEADER");
                }
            }
            _ => {}
        }
    }
}

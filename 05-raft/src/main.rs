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
        #[allow(dead_code)] // read in Step 2: log matching + the commit-term safety rule
        term: u64,
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

    fn port_of(addr: &str) -> u16 {
        addr.rsplit(':')
            .next()
            .and_then(|p| p.parse().ok())
            .unwrap_or(u16::MAX)
    }

    // RequestVote RPC: ask a peer for its vote. Some((their_term, granted)) or None if unreachable.
    fn request_vote(peer: &str, term: u64, me: &str) -> Option<(u64, bool)> {
        let mut conn = TcpStream::connect(peer).ok()?;
        conn.write_all(format!("requestvote {term} {me}\n").as_bytes())
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
        let mut conn = TcpStream::connect(peer).ok()?;
        conn.write_all(format!("append {term} {leader} {commit} {entries}\n").as_bytes())
            .ok()?;
        let mut reply = String::new();
        BufReader::new(&conn).read_line(&mut reply).ok()?;
        match reply.split_whitespace().collect::<Vec<_>>().as_slice() {
            ["appendack", t, len] => Some((t.parse().ok()?, len.parse().ok()?)),
            _ => None,
        }
    }

    fn main() {
        let args: Vec<String> = std::env::args().collect();
        let port = args.get(1).cloned().unwrap_or_else(|| "6000".to_string());
        let peers: Vec<String> = args.iter().skip(2).cloned().collect();
        let me = format!("127.0.0.1:{port}");
        let total = peers.len() + 1;
        let majority = total / 2 + 1;
        let election_timeout = Duration::from_millis(1500 + (port_of(&me) as u64 % 7) * 300);
        println!("node {me} — peers {peers:?} — timeout {election_timeout:?}, majority {majority}");

        let state = Arc::new(Mutex::new(State {
            term: 0,
            role: Role::Follower,
            voted_for: None,
            last_heard: Instant::now(),
            log: Vec::new(),
            commit_index: 0,
            applied: 0,
            kv: std::collections::HashMap::new(),
        }));

        // ---- election + heartbeat thread ----
        {
            let me = me.clone();
            let peers = peers.clone();
            let state = Arc::clone(&state);
            thread::spawn(move || loop {
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
                    // AppendEntries to every peer; collect each one's log length (0 if unreachable)
                    let mut lengths = vec![self_len]; // self counts
                    for peer in &peers {
                        let len = append_entries(peer, term, &me, commit, &blob)
                            .map(|(_, l)| l)
                            .unwrap_or(0);
                        lengths.push(len);
                    }
                    let majority = (lengths.len() / 2) + 1; // majority count
                    lengths.sort_unstable_by(|a, b| b.cmp(a));   // descending
                    let agreed = lengths[majority - 1];          // the majority-th largest
                    let mut s = state.lock().unwrap();
                    if agreed > s.commit_index { s.commit_index = agreed; }
                    apply(&mut s);
                    continue;
                }

                // Follower/candidate: has the leader gone silent past our timeout?
                let timed_out = { state.lock().unwrap().last_heard.elapsed() > election_timeout };
                if !timed_out {
                    continue;
                }

                // Become a candidate for a NEW term (short lock), snapshot the term.
                let term = {
                    let mut s = state.lock().unwrap();
                    s.term += 1;
                    s.role = Role::Candidate;
                    s.voted_for = Some(me.clone());
                    s.last_heard = Instant::now();
                    println!("term {}: {me} → CANDIDATE (requesting votes)", s.term);
                    s.term
                };

                let total = peers.len() + 1; // including self
                let votes: usize = {
                    let mut count = 1; // self-vote
                    for peer in &peers {
                        if let Some((_, granted)) = request_vote(peer, term, &me) {
                            if granted {
                                count += 1;
                            }
                        }
                    }
                    count
                };

                // Won? Re-lock and confirm we're STILL a candidate in the SAME term.
                let mut s = state.lock().unwrap();
                if s.role == Role::Candidate && s.term == term && votes >= majority {
                    s.role = Role::Leader;
                    println!("term {term}: {me} → LEADER ({votes}/{total} votes)");
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
            let mut reader = BufReader::new(stream);
            let mut line = String::new();
            if reader.read_line(&mut line).is_err() {
                continue;
            }

            match line.split_whitespace().collect::<Vec<_>>().as_slice() {
                ["requestvote", term, candidate] => {
                    let cand_term: u64 = term.parse().unwrap_or(0);
                    let candidate = candidate.to_string();
                    let (my_term, granted) = {
                        let mut s = state.lock().unwrap();
                        if cand_term > s.term {
                            s.term = cand_term;
                            s.role = Role::Follower;
                            s.voted_for = None;
                        }
                        let granted = cand_term == s.term
                            && (s.voted_for.is_none()
                                || s.voted_for.as_deref() == Some(candidate.as_str()));
                        if granted {
                            s.voted_for = Some(candidate.clone());
                            s.last_heard = Instant::now();
                        }
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
                            s.commit_index = leader_commit.min(s.log.len());
                            apply(&mut s);  
                        }
                        (s.term, s.log.len())
                    };
                    let _ = writeln!(writer, "appendack {my_term} {my_len}");
                }
                ["set", ..] | ["remove", ..] => {
                    let mut s = state.lock().unwrap();
                    if s.role == Role::Leader {
                        let term = s.term; // read first (ends the borrow) — the guard Derefs the WHOLE State
                        s.log.push(Entry {
                            term,
                            cmd: line.trim().to_string(),
                        });
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

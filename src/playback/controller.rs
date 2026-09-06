//! E16 wire adapter. Musical edits live in crate::control, independent of MIDI.
use crate::control::{PARAMETERS, Parameter, Shared, View};
// E16 layout is independent of the engine parameter registry.
const LAYOUT: [usize; 16] = [4, 5, 6, 2, 8, 9, 10, 7, 12, 13, 14, 11, 0, 1, 3, 15];
use serde::Serialize;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
    mpsc,
};
use std::time::Duration;
const PREFIX: &[u8] = &[0xf0, 0x7d, 0x50, 0x43, 1];
#[derive(Clone, Default, Serialize)]
pub struct Status {
    pub connected: bool,
    pub received: u64,
    pub dropped: u64,
    pub error: Option<String>,
    pub view: Option<View>,
    pub parts: Vec<View>,
}
pub struct Connection {
    running: Arc<AtomicBool>,
    worker: Option<std::thread::JoinHandle<()>>,
    status: Arc<Mutex<Status>>,
}
impl Drop for Connection {
    fn drop(&mut self) {
        self.running.store(false, Ordering::Relaxed);
        if let Some(w) = self.worker.take() {
            let _ = w.join();
        }
    }
}
impl Connection {
    pub fn status(&self) -> Status {
        self.status.lock().unwrap().clone()
    }
}
pub fn inputs() -> Result<Vec<String>, String> {
    let m = midir::MidiInput::new("Phasecraft controls").map_err(|e| e.to_string())?;
    m.ports()
        .iter()
        .map(|p| m.port_name(p).map_err(|e| e.to_string()))
        .collect()
}
#[derive(Debug, PartialEq)]
enum Message {
    Hello(u8),
    KitHello(u8),
    Select {
        generation: u16,
        page: u8,
        slot: usize,
    },
    Turn {
        generation: u16,
        page: u8,
        slot: usize,
        delta: i32,
    },
    Reset {
        generation: u16,
    },
}
fn decode(b: &[u8]) -> Option<Message> {
    if !b.starts_with(PREFIX)
        || b.last() != Some(&0xf7)
        || b[5..b.len() - 1].iter().any(|v| *v > 127)
    {
        return None;
    }
    match (b.get(5)?, b.len()) {
        (6, 8) if (1..=3).contains(&b[6]) => Some(Message::KitHello(b[6])),
        (5, 11) if matches!(b[8], 1 | 3) && b[9] < 16 => Some(Message::Select {
            generation: u16::from(b[6]) * 128 + u16::from(b[7]),
            page: b[8],
            slot: b[9] as usize,
        }),
        (1, 8) if (1..=2).contains(&b[6]) => Some(Message::Hello(b[6])),
        (2, 12) if (1..=3).contains(&b[8]) && b[9] < 16 && (56..=72).contains(&b[10]) => {
            Some(Message::Turn {
                generation: u16::from(b[6]) * 128 + u16::from(b[7]),
                page: b[8],
                slot: b[9] as usize,
                delta: i32::from(b[10]) - 64,
            })
        }
        (3, 9) => Some(Message::Reset {
            generation: u16::from(b[6]) * 128 + u16::from(b[7]),
        }),
        _ => None,
    }
}
fn packet(kind: u8, body: &[u8]) -> Vec<u8> {
    let mut v = PREFIX.to_vec();
    v.push(kind);
    v.extend(body);
    v.push(0xf7);
    v
}
fn feedback(view: &View, page: u8) -> Vec<Vec<u8>> {
    (0..16)
        .map(|i| {
            let v = &view.values[if page == 1 { 0 } else { LAYOUT[i] }];
            let enabled = v.enabled && (page == 2 || i == 0);
            let value = if enabled && v.maximum > 0. {
                (v.value / v.maximum * 16383.).round() as u16
            } else {
                0
            };
            let label = if page == 1 && i == 0 {
                "Kick"
            } else if enabled {
                v.label.as_str()
            } else {
                "----"
            };
            let mut body = vec![
                (view.generation / 128) as u8,
                (view.generation % 128) as u8,
                page,
                i as u8,
                u8::from(enabled),
                (value / 128) as u8,
                (value % 128) as u8,
            ];
            body.extend(label.bytes().take(4));
            while body.len() < 11 {
                body.push(b' ');
            }
            let text = if !enabled {
                "----".into()
            } else if matches!(v.parameter, Parameter::Operator) {
                ["A", "OR", "AND", "XOR", "A-B", "B-A"][v.value as usize].into()
            } else if matches!(
                v.parameter,
                Parameter::Level
                    | Parameter::Cutoff
                    | Parameter::Decay
                    | Parameter::TriggerProbability
                    | Parameter::AccentProbability
                    | Parameter::AccentAmount
            ) {
                format!("{}%", (v.value * 100.).round() as i32)
            } else if v.value >= 10000. {
                format!("{}k", (v.value / 1000.).round() as i32)
            } else {
                format!("{}", v.value as i32)
            };
            body.extend(text.bytes().take(4));
            while body.len() < 15 {
                body.push(b' ');
            }
            packet(4, &body)
        })
        .collect()
}
fn labels(parts: &[View]) -> Vec<String> {
    let mut result: Vec<String> = parts
        .iter()
        .map(|p| match p.part.as_str() {
            "kick" => "Kick".into(),
            "closed_hat" => "CHat".into(),
            "open_hat" => "OHat".into(),
            "snare" => "Snar".into(),
            _ => p
                .part
                .chars()
                .filter(|c| c.is_ascii_alphanumeric())
                .take(4)
                .collect::<String>(),
        })
        .collect();
    let original = result.clone();
    for (i, label) in result.iter_mut().enumerate() {
        if label.is_empty()
            || original
                .iter()
                .filter(|v| v.eq_ignore_ascii_case(label))
                .count()
                > 1
        {
            *label = format!("P{:02}", i + 1);
        }
    }
    // Reserve numeric fallbacks too: authored IDs may themselves look like P01.
    if (0..result.len()).any(|i| {
        result[i + 1..]
            .iter()
            .any(|v| v.eq_ignore_ascii_case(&result[i]))
    }) {
        result = (0..parts.len()).map(|i| format!("P{:02}", i + 1)).collect();
    }
    result
}
fn kit_feedback(selected: &View, parts: &[View], page: u8) -> Vec<Vec<u8>> {
    let names = labels(parts);
    let mut packets = feedback(selected, 2);
    for (i, packet) in packets.iter_mut().enumerate() {
        packet[8] = page;
        if page == 2 {
            packet[10] = u8::from(selected.values[LAYOUT[i]].enabled)
                | (u8::from(selected.values[LAYOUT[i]].pending) << 1);
        } else {
            let index = i + if page == 3 { 16 } else { 0 };
            if let Some(part) = parts.get(index) {
                *packet = feedback(part, 1).remove(0);
                packet[8] = page;
                packet[9] = i as u8;
                packet[10] = u8::from(part.values[0].enabled)
                    | (u8::from(part.pending) << 1)
                    | (u8::from(part.selected) << 2)
                    | 8;
                for (j, c) in names[index]
                    .bytes()
                    .chain(std::iter::repeat(b' '))
                    .take(4)
                    .enumerate()
                {
                    packet[13 + j] = c;
                }
            } else {
                packet[10] = 0;
                packet[11] = 0;
                packet[12] = 0;
                packet[13..21].copy_from_slice(b"--------");
            }
        }
    }
    let title = if page == 2 {
        format!(
            "{}{}",
            if selected.pending { "* " } else { "" },
            selected.part
        )
    } else {
        format!("Kit {} / {}", if page == 3 { 2 } else { 1 }, selected.part)
    };
    let mut body = vec![
        (selected.generation / 128) as u8,
        (selected.generation % 128) as u8,
        page,
    ];
    body.extend(
        title
            .chars()
            .map(|c| {
                if c.is_ascii_graphic() || c == ' ' {
                    c as u8
                } else {
                    b'?'
                }
            })
            .take(15),
    );
    while body.len() < 18 {
        body.push(b' ');
    }
    packets.push(packet(7, &body));
    packets
}
fn apply(
    live: &mut crate::control::Live,
    message: Message,
    page: &mut u8,
    kit: &mut bool,
) -> Result<bool, String> {
    match message {
        Message::KitHello(p) => {
            *page = p;
            *kit = true;
            return Ok(true);
        }
        Message::Select {
            generation,
            page: p,
            slot,
        } => {
            if *kit
                && generation == live.generation
                && let Some(id) = live.ids().get(slot + if p == 3 { 16 } else { 0 })
            {
                live.select(id)?;
                *page = p;
            }
        }
        Message::Hello(p) => {
            *kit = false;
            *page = p;
            return Ok(true);
        }
        Message::Turn {
            generation,
            page: p,
            slot,
            delta,
        } => {
            if generation == live.generation && (*kit || p == 2 || (p == 1 && slot == 0)) {
                *page = p;
                let id = if !*kit {
                    Some("kick".to_string())
                } else if p == 2 {
                    Some(live.selected.clone())
                } else {
                    live.ids().get(slot + if p == 3 { 16 } else { 0 }).cloned()
                };
                if let Some(id) = id {
                    live.change(
                        &id,
                        PARAMETERS[if p != 2 { 0 } else { LAYOUT[slot] }],
                        delta,
                    )?;
                }
            }
        }
        Message::Reset { generation } => {
            if generation == live.generation {
                live.reset();
            }
        }
    }
    Ok(false)
}
pub fn connect(input: String, output: String, live: Shared) -> Result<Connection, String> {
    let mut midi = midir::MidiInput::new("Phasecraft controls").map_err(|e| e.to_string())?;
    midi.ignore(midir::Ignore::None);
    let port = midi
        .ports()
        .into_iter()
        .find(|p| midi.port_name(p).ok().as_deref() == Some(&input))
        .ok_or("Controller input not found")?;
    let mut out = super::ports::open_output(Some(output), false)?;
    let (tx, rx) = mpsc::sync_channel(128);
    let dropped = Arc::new(std::sync::atomic::AtomicU64::new(0));
    let drop_count = dropped.clone();
    let input = midi
        .connect(
            &port,
            "Phasecraft controls",
            move |_, bytes, _| {
                if let Some(message) = decode(bytes)
                    && tx.try_send(message).is_err()
                {
                    drop_count.fetch_add(1, Ordering::Relaxed);
                }
            },
            (),
        )
        .map_err(|e| e.to_string())?;
    let running = Arc::new(AtomicBool::new(true));
    let run = running.clone();
    let status = Arc::new(Mutex::new(Status {
        connected: true,
        ..Default::default()
    }));
    let stat = status.clone();
    let worker = std::thread::spawn(move || {
        let _input = input;
        let mut page = 1;
        let mut kit = false;
        let mut last = Vec::new();
        let mut received = 0;
        while run.load(Ordering::Relaxed) {
            let message = match rx.recv_timeout(Duration::from_millis(40)) {
                Ok(m) => Some(m),
                Err(mpsc::RecvTimeoutError::Timeout) => None,
                Err(_) => break,
            };
            let mut messages = Vec::new();
            if let Some(m) = message {
                messages.push(m);
            }
            messages.extend(rx.try_iter().take(127));
            let started = std::time::Instant::now();
            let mut error = None;
            let (view, parts) = {
                let mut live = live.lock().unwrap();
                for message in messages {
                    received += 1;
                    match apply(&mut live, message, &mut page, &mut kit) {
                        Ok(true) => last.clear(),
                        Ok(false) => {}
                        Err(e) => error = Some(e),
                    }
                }
                (
                    live.view(if kit { &live.selected } else { "kick" }),
                    live.views(),
                )
            };
            let packets = if kit {
                kit_feedback(&view, &parts, page)
            } else {
                feedback(&view, page)
            };
            if packets != last {
                for p in &packets {
                    if let Err(e) = out.send(p) {
                        error = Some(e.to_string());
                        run.store(false, Ordering::Relaxed);
                        break;
                    }
                }
                last = packets;
            }
            let mut s = stat.lock().unwrap();
            s.connected = run.load(Ordering::Relaxed);
            s.received = received;
            s.dropped = dropped.load(Ordering::Relaxed);
            if error.is_some() {
                s.error = error;
            }
            s.view = Some(view);
            s.parts = parts;
            drop(s);
            std::thread::sleep(Duration::from_millis(40).saturating_sub(started.elapsed()));
        }
        stat.lock().unwrap().connected = false;
    });
    Ok(Connection {
        running,
        worker: Some(worker),
        status,
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn kit_selects_parts_and_banks_without_losing_other_edits() {
        let c=crate::music::Composition::parse("tempo=132\nseed=1\n[parts.kick]\nuse='techno.kick'\n[parts.snare]\nuse='techno.clap'\n").unwrap();
        let mut live = crate::control::Live::default();
        live.load(Some(c.clone()));
        let mut page = 1;
        let mut kit = false;
        apply(&mut live, Message::KitHello(1), &mut page, &mut kit).unwrap();
        let generation = live.generation;
        apply(
            &mut live,
            Message::Select {
                generation,
                page: 1,
                slot: 1,
            },
            &mut page,
            &mut kit,
        )
        .unwrap();
        assert_eq!(live.selected, "snare");
        let generation = live.generation;
        apply(
            &mut live,
            Message::Turn {
                generation,
                page: 2,
                slot: 3,
                delta: -20,
            },
            &mut page,
            &mut kit,
        )
        .unwrap();
        assert_eq!(live.view("snare").values[2].value, 0.8);
        assert_eq!(live.view("kick").values[2].value, 1.);
        let f = kit_feedback(&live.view("snare"), &live.views(), 1);
        assert_eq!(&f[0][13..17], b"Kick");
        assert_eq!(&f[1][13..17], b"Snar");
        assert_eq!(f[1][10] & 12, 12); // present and selected, even without level mapping
        assert_eq!(f[2][10], 0);
        let mut many = c.clone();
        many.parts = (0..32)
            .map(|i| {
                let mut p = c.parts[0].clone();
                p.id = format!("part_{i}");
                p.output.note = i as u8;
                p
            })
            .collect();
        live.load(Some(many));
        let generation = live.generation;
        apply(
            &mut live,
            Message::Select {
                generation,
                page: 3,
                slot: 15,
            },
            &mut page,
            &mut kit,
        )
        .unwrap();
        assert_eq!(live.selected, "part_31");
        let f = kit_feedback(&live.view(&live.selected), &live.views(), 3);
        assert!(f.iter().all(|p| p[1..p.len() - 1].iter().all(|v| *v < 128)));
        assert_eq!(f[15][10] & 12, 12);
        let names = labels(&live.views());
        assert_eq!(
            names
                .iter()
                .collect::<std::collections::BTreeSet<_>>()
                .len(),
            32
        );
    }
    #[test]
    fn kick_layout_round_trip_and_stale_commands() {
        let mut live = crate::control::Live::default();
        live.load(Some(
            crate::authoring::project::load(std::path::Path::new(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/examples/controllers/kick"
            )))
            .unwrap()
            .composition,
        ));
        let mut page = 1;
        let mut kit = false;
        let generation = live.generation;
        let turn = |generation, page, slot| Message::Turn {
            generation,
            page,
            slot,
            delta: -1,
        };
        apply(&mut live, turn(generation, 1, 0), &mut page, &mut kit).unwrap();
        assert_eq!(live.view("kick").values[0].value, 0.99);
        apply(&mut live, turn(generation, 2, 0), &mut page, &mut kit).unwrap();
        assert_eq!(live.view("kick").values[4].value, 15.);
        let packets = feedback(&live.view("kick"), 2);
        assert_eq!(&packets[0][13..17], b"ASte");
        assert_eq!(&packets[0][17..21], b"15  ");
        assert!(
            packets
                .iter()
                .all(|p| p.len() == 22 && p[1..21].iter().all(|v| *v < 128))
        );
        live.reset();
        apply(&mut live, turn(generation, 2, 0), &mut page, &mut kit).unwrap();
        assert!(!live.view("kick").edited);
        let generation = live.generation;
        apply(&mut live, turn(generation, 1, 1), &mut page, &mut kit).unwrap();
        assert!(!live.view("kick").edited);
    }
    #[test]
    fn wire_rejects_foreign_truncated_and_out_of_range() {
        assert_eq!(decode(&packet(1, &[2])), Some(Message::Hello(2)));
        assert!(decode(&[0xf0, 0x7d]).is_none());
        assert!(decode(&packet(2, &[0, 1, 2, 16, 65])).is_none());
        assert!(decode(&packet(2, &[0, 1, 2, 0, 100])).is_none());
        assert_eq!(
            decode(&packet(2, &[0, 1, 2, 0, 63])),
            Some(Message::Turn {
                generation: 1,
                page: 2,
                slot: 0,
                delta: -1
            })
        );
    }
}

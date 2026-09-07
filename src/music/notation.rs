//! Step notation: the literal rhythm/value leaf. `docs/notation.md` is the contract, and the
//! grammar there is closed — a construct the spec does not list is a parse error, not a feature.
//! Positions are exact rationals over one cycle; ticks are taken only at the very end, by floor.
use std::cmp::Ordering;

/// An exact fraction of one cycle, always reduced.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct Ratio {
    pub num: u64,
    pub den: u64,
}
impl Ratio {
    pub const ZERO: Self = Self { num: 0, den: 1 };
    pub const ONE: Self = Self { num: 1, den: 1 };
    /// Reduced `num/den`; a zero denominator is a caller bug.
    pub fn new(num: u64, den: u64) -> Self {
        Self::reduce(u128::from(num), u128::from(den))
    }
    pub fn is_zero(self) -> bool {
        self.num == 0
    }
    /// `floor(cycle_ticks * num / den)`, computed in `u128` so the product cannot wrap.
    pub fn ticks(self, cycle_ticks: u64) -> u64 {
        let t = u128::from(cycle_ticks) * u128::from(self.num) / u128::from(self.den);
        u64::try_from(t).unwrap_or(u64::MAX)
    }
    /// `self * n`, the integer multiply.
    pub fn mul_int(self, n: u64) -> Self {
        Self::reduce(u128::from(self.num) * u128::from(n), u128::from(self.den))
    }
    fn reduce(num: u128, den: u128) -> Self {
        assert!(den != 0, "ratio denominator is zero");
        let g = gcd(num, den);
        // `Pattern::parse` proves every denominator a pattern can reach fits in u64.
        Self {
            num: u64::try_from(num / g).expect("ratio numerator exceeds u64"),
            den: u64::try_from(den / g).expect("ratio denominator exceeds u64"),
        }
    }
}
impl std::ops::Add for Ratio {
    type Output = Self;
    fn add(self, rhs: Self) -> Self {
        Self::reduce(
            u128::from(self.num) * u128::from(rhs.den) + u128::from(rhs.num) * u128::from(self.den),
            u128::from(self.den) * u128::from(rhs.den),
        )
    }
}
impl std::ops::Mul for Ratio {
    type Output = Self;
    fn mul(self, rhs: Self) -> Self {
        Self::reduce(
            u128::from(self.num) * u128::from(rhs.num),
            u128::from(self.den) * u128::from(rhs.den),
        )
    }
}
impl std::ops::Mul<u64> for Ratio {
    type Output = Self;
    fn mul(self, rhs: u64) -> Self {
        self.mul_int(rhs)
    }
}
impl Ord for Ratio {
    fn cmp(&self, other: &Self) -> Ordering {
        (u128::from(self.num) * u128::from(other.den))
            .cmp(&(u128::from(other.num) * u128::from(self.den)))
    }
}
impl PartialOrd for Ratio {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
fn gcd(a: u128, b: u128) -> u128 {
    let (mut a, mut b) = (a, b);
    while b != 0 {
        (a, b) = (b, a % b);
    }
    if a == 0 { 1 } else { a }
}
fn lcm(a: u128, b: u128) -> Option<u128> {
    if a == 0 || b == 0 {
        return None;
    }
    (a / gcd(a, b)).checked_mul(b)
}
fn lcm64(a: u64, b: u64) -> Option<u64> {
    u64::try_from(lcm(u128::from(a), u128::from(b))?).ok()
}

/// What a step says. Rests never reach an `Event`.
#[derive(Clone, Debug, PartialEq)]
pub enum Token {
    Hit,
    Number(f64),
    Note { name: String, midi: u8 },
    Name(String),
}
/// Where a `?` puts its draw: on the hit itself, or on a ratchet's tail.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Draw {
    None,
    Hit,
    Tail,
}
/// One sounding position in a cycle, in exact cycle fractions.
#[derive(Clone, Debug, PartialEq)]
pub struct Event {
    pub onset: Ratio,
    pub span: Ratio,
    pub token: Token,
    /// Ratchet count over `span`; 1 means no ratchet.
    pub ratchet: u8,
    pub draw: Draw,
}
/// The same event placed on the tick grid, tails resolved.
#[derive(Clone, Debug, PartialEq)]
pub struct TickEvent {
    pub tick: u64,
    pub span_ticks: u64,
    pub token: Token,
    pub ratchet: u8,
    pub draw: Draw,
    pub tails: Vec<u64>,
}
/// Which token kinds a pattern contains, so a consumer can refuse a mismatch at load.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct TokenKinds {
    pub hits: bool,
    pub numbers: bool,
    pub notes: bool,
    pub names: bool,
}

#[derive(Clone, Copy, Debug)]
struct Euclid {
    pulses: u64,
    steps: u64,
    rotate: i64,
}
impl Euclid {
    /// Balanced modular rule, identical to `rhythm::Expression::Euclidean`.
    fn active(self, i: u64) -> bool {
        let idx = (i as i64 - self.rotate).rem_euclid(self.steps as i64) as u64;
        (idx * self.pulses) % self.steps < self.pulses
    }
}
#[derive(Clone, Debug)]
enum Atom {
    Rest,
    Token(Token),
    /// `[ ]` — one or more sequences over the same span.
    Stack(Vec<Vec<Step>>),
    /// `< >` — element `k mod n` takes the whole span.
    Alt(Vec<Step>),
    /// `{ }%n` — `per_cycle` elements per span, element `(k*n + j) mod m`.
    Poly {
        steps: Vec<Step>,
        per_cycle: u64,
    },
}
#[derive(Clone, Debug)]
struct Step {
    atom: Atom,
    weight: Option<Ratio>,
    ratchet: Option<u8>,
    draw: bool,
    euclid: Option<Euclid>,
}
impl Step {
    fn weight(&self) -> Ratio {
        self.weight.unwrap_or(Ratio::ONE)
    }
}
/// Modifiers a composite step hands to the leaves inside it; the inner step's own value wins.
#[derive(Clone, Copy, Default)]
struct Inherit {
    ratchet: Option<u8>,
    draw: bool,
}

/// One cycle of material, parsed once and rendered per cycle index.
#[derive(Clone, Debug)]
pub struct Pattern {
    stack: Vec<Vec<Step>>,
}
impl Pattern {
    pub fn parse(text: &str) -> Result<Self, String> {
        let mut p = Parser { src: text, pos: 0 };
        let stack = p.stack(0)?;
        p.skip_ws();
        if p.pos < text.len() {
            return p.fail(&format!("unexpected {}", p.here()));
        }
        for seq in &stack {
            check_bounds(seq, 1)?;
        }
        Ok(Self { stack })
    }
    /// Events of cycle `k`, sorted by onset; simultaneous events keep source order.
    pub fn cycle(&self, k: u64) -> Vec<Event> {
        let mut out = Vec::new();
        for seq in &self.stack {
            render_seq(
                seq,
                Ratio::ZERO,
                Ratio::ONE,
                k,
                Inherit::default(),
                &mut out,
            );
        }
        out.sort_by_key(|e| e.onset);
        out
    }
    /// Cycle `k` on the tick grid: `tick = k * cycle_ticks + floor(cycle_ticks * onset)`.
    pub fn ticks(&self, k: u64, cycle_ticks: u64) -> Vec<TickEvent> {
        let base = k.saturating_mul(cycle_ticks);
        self.cycle(k)
            .into_iter()
            .map(|e| {
                let onset = e.onset.ticks(cycle_ticks);
                let span_ticks = (e.onset + e.span).ticks(cycle_ticks).saturating_sub(onset);
                let tick = base.saturating_add(onset);
                let n = u64::from(e.ratchet).max(1);
                let tails = (1..n)
                    .map(|i| tick + span_ticks / n * i + span_ticks % n * i / n)
                    .collect();
                TickEvent {
                    tick,
                    span_ticks,
                    token: e.token,
                    ratchet: e.ratchet,
                    draw: e.draw,
                    tails,
                }
            })
            .collect()
    }
    pub fn kinds(&self) -> TokenKinds {
        let mut kinds = TokenKinds::default();
        for seq in &self.stack {
            seq_kinds(seq, &mut kinds);
        }
        kinds
    }
    /// Cycles after which the pattern repeats: lcm of every `< >` length and `{ }%n` period.
    /// None when that lcm exceeds u64.
    pub fn period_cycles(&self) -> Option<u64> {
        let mut acc = 1;
        for seq in &self.stack {
            acc = seq_period(seq, acc)?;
        }
        Some(acc)
    }
}

fn seq_kinds(steps: &[Step], kinds: &mut TokenKinds) {
    for s in steps {
        match &s.atom {
            Atom::Rest => {}
            Atom::Token(Token::Hit) => kinds.hits = true,
            Atom::Token(Token::Number(_)) => kinds.numbers = true,
            Atom::Token(Token::Note { .. }) => kinds.notes = true,
            Atom::Token(Token::Name(_)) => kinds.names = true,
            Atom::Stack(seqs) => {
                for q in seqs {
                    seq_kinds(q, kinds);
                }
            }
            Atom::Alt(elems) | Atom::Poly { steps: elems, .. } => seq_kinds(elems, kinds),
        }
    }
}
fn seq_period(steps: &[Step], acc: u64) -> Option<u64> {
    let mut acc = acc;
    for s in steps {
        acc = step_period(s, acc)?;
    }
    Some(acc)
}
fn step_period(step: &Step, acc: u64) -> Option<u64> {
    match &step.atom {
        Atom::Rest | Atom::Token(_) => Some(acc),
        Atom::Stack(seqs) => {
            let mut acc = acc;
            for q in seqs {
                acc = seq_period(q, acc)?;
            }
            Some(acc)
        }
        Atom::Alt(elems) => seq_period(elems, lcm64(acc, elems.len() as u64)?),
        Atom::Poly { steps, per_cycle } => {
            let own = lcm64(steps.len() as u64, *per_cycle)? / *per_cycle;
            seq_period(steps, lcm64(acc, own)?)
        }
    }
}

/// Integer shares per step and their total, over the sequence's common weight denominator.
fn weights(steps: &[Step]) -> Option<(Vec<u128>, u128)> {
    let mut den: u128 = 1;
    for s in steps {
        den = lcm(den, u128::from(s.weight().den))?;
    }
    let mut shares = Vec::with_capacity(steps.len());
    let mut total: u128 = 0;
    for s in steps {
        let w = s.weight();
        let share = u128::from(w.num).checked_mul(den / u128::from(w.den))?;
        total = total.checked_add(share)?;
        shares.push(share);
    }
    (total > 0).then_some((shares, total))
}
/// Prove no reachable onset or span needs a denominator past u64, so rendering cannot panic.
fn check_bounds(steps: &[Step], acc: u64) -> Result<(), String> {
    let (_, total) = weights(steps).ok_or_else(|| "step weights overflow".to_string())?;
    let total = u64::try_from(total).map_err(|_| "step weights overflow".to_string())?;
    let acc = mul_bound(acc, total)?;
    for s in steps {
        check_step_bounds(s, acc)?;
    }
    Ok(())
}
fn check_step_bounds(step: &Step, acc: u64) -> Result<(), String> {
    let acc = match step.euclid {
        Some(e) => mul_bound(acc, e.steps)?,
        None => acc,
    };
    match &step.atom {
        Atom::Rest | Atom::Token(_) => {}
        Atom::Stack(seqs) => {
            for q in seqs {
                check_bounds(q, acc)?;
            }
        }
        Atom::Alt(elems) => {
            for e in elems {
                check_step_bounds(e, acc)?;
            }
        }
        Atom::Poly { steps, per_cycle } => {
            let acc = mul_bound(acc, *per_cycle)?;
            for e in steps {
                check_step_bounds(e, acc)?;
            }
        }
    }
    Ok(())
}
fn mul_bound(acc: u64, factor: u64) -> Result<u64, String> {
    acc.checked_mul(factor)
        .ok_or_else(|| "pattern subdivides too finely for exact u64 positions".to_string())
}

fn render_seq(
    steps: &[Step],
    start: Ratio,
    span: Ratio,
    k: u64,
    inh: Inherit,
    out: &mut Vec<Event>,
) {
    let (shares, total) = weights(steps).expect("weights proved finite by parse");
    let mut prefix: u128 = 0;
    for (step, share) in steps.iter().zip(&shares) {
        let sub_start = start + span * Ratio::reduce(prefix, total);
        let sub_span = span * Ratio::reduce(*share, total);
        render_step(step, sub_start, sub_span, k, inh, out);
        prefix += share;
    }
}
fn render_step(step: &Step, start: Ratio, span: Ratio, k: u64, inh: Inherit, out: &mut Vec<Event>) {
    let inh = Inherit {
        ratchet: step.ratchet.or(inh.ratchet),
        draw: step.draw || inh.draw,
    };
    let Some(e) = step.euclid else {
        return render_atom(&step.atom, start, span, k, inh, out);
    };
    let cell = span * Ratio::new(1, e.steps);
    for i in 0..e.steps {
        if e.active(i) {
            render_atom(&step.atom, start + cell.mul_int(i), cell, k, inh, out);
        }
    }
}
fn render_atom(atom: &Atom, start: Ratio, span: Ratio, k: u64, inh: Inherit, out: &mut Vec<Event>) {
    match atom {
        Atom::Rest => {}
        Atom::Token(token) => out.push(Event {
            onset: start,
            span,
            token: token.clone(),
            ratchet: inh.ratchet.unwrap_or(1),
            draw: match (inh.draw, inh.ratchet) {
                (false, _) => Draw::None,
                (true, None) => Draw::Hit,
                (true, Some(_)) => Draw::Tail,
            },
        }),
        Atom::Stack(seqs) => {
            for seq in seqs {
                render_seq(seq, start, span, k, inh, out);
            }
        }
        Atom::Alt(elems) => {
            let i = (k % elems.len() as u64) as usize;
            render_step(&elems[i], start, span, k, inh, out);
        }
        Atom::Poly { steps, per_cycle } => {
            let (n, m) = (*per_cycle, steps.len() as u128);
            let cell = span * Ratio::new(1, n);
            for j in 0..n {
                let i = ((u128::from(k) * u128::from(n) + u128::from(j)) % m) as usize;
                render_step(&steps[i], start + cell.mul_int(j), cell, k, inh, out);
            }
        }
    }
}

const MAX_DEPTH: usize = 8;
const MAX_STEPS: u64 = 1024;

struct Parser<'a> {
    src: &'a str,
    pos: usize,
}
impl Parser<'_> {
    fn fail<T>(&self, what: &str) -> Result<T, String> {
        Err(format!("at byte {}: {what}", self.pos))
    }
    fn here(&self) -> String {
        match self.src[self.pos..].chars().next() {
            Some(c) => format!("{c:?}"),
            None => "end of pattern".to_string(),
        }
    }
    fn peek(&self) -> Option<u8> {
        self.src.as_bytes().get(self.pos).copied()
    }
    fn skip_ws(&mut self) {
        while matches!(self.peek(), Some(c) if c.is_ascii_whitespace()) {
            self.pos += 1;
        }
    }
    fn expect(&mut self, b: u8) -> Result<(), String> {
        self.skip_ws();
        if self.peek() == Some(b) {
            self.pos += 1;
            return Ok(());
        }
        self.fail(&format!("expected {:?}, found {}", b as char, self.here()))
    }
    fn integer(&mut self) -> Result<u64, String> {
        let start = self.pos;
        let mut value: u64 = 0;
        while let Some(c) = self.peek().filter(u8::is_ascii_digit) {
            value = value
                .checked_mul(10)
                .and_then(|v| v.checked_add(u64::from(c - b'0')))
                .ok_or_else(|| format!("at byte {start}: whole number is too large"))?;
            self.pos += 1;
        }
        if self.pos == start {
            return self.fail(&format!("expected a whole number, found {}", self.here()));
        }
        Ok(value)
    }
    /// A stack: one or more comma-separated sequences over the same span.
    fn stack(&mut self, depth: usize) -> Result<Vec<Vec<Step>>, String> {
        let mut seqs = vec![self.sequence(depth)?];
        loop {
            self.skip_ws();
            if self.peek() != Some(b',') {
                return Ok(seqs);
            }
            self.pos += 1;
            seqs.push(self.sequence(depth)?);
        }
    }
    fn sequence(&mut self, depth: usize) -> Result<Vec<Step>, String> {
        let mut steps = Vec::new();
        loop {
            self.skip_ws();
            match self.peek() {
                None | Some(b']' | b'>' | b'}' | b',') => break,
                _ => steps.push(self.step(depth)?),
            }
        }
        if steps.is_empty() {
            return self.fail(&format!("expected a step, found {}", self.here()));
        }
        Ok(steps)
    }
    fn step(&mut self, depth: usize) -> Result<Step, String> {
        let atom = self.atom(depth)?;
        let mut step = Step {
            atom,
            weight: None,
            ratchet: None,
            draw: false,
            euclid: None,
        };
        loop {
            match self.peek() {
                Some(b'*') => {
                    if step.ratchet.is_some() {
                        return self.fail("'*' already given on this step");
                    }
                    self.pos += 1;
                    let n = self.integer()?;
                    if !(2..=8).contains(&n) {
                        return self.fail("expected a ratchet count 2..=8");
                    }
                    step.ratchet = Some(n as u8);
                }
                Some(b'@') => {
                    if step.weight.is_some() {
                        return self.fail("'@' already given on this step");
                    }
                    self.pos += 1;
                    step.weight = Some(self.weight()?);
                }
                Some(b'?') => {
                    if step.draw {
                        return self.fail("'?' already given on this step");
                    }
                    self.pos += 1;
                    step.draw = true;
                }
                Some(b'(') => {
                    if step.euclid.is_some() {
                        return self.fail("euclid already given on this step");
                    }
                    self.pos += 1;
                    step.euclid = Some(self.euclid()?);
                }
                _ => break,
            }
        }
        // The grammar separates steps by whitespace; anything else glued on is an error.
        if !ends_step(self.peek()) {
            return self.fail(&format!(
                "expected whitespace between steps, found {}",
                self.here()
            ));
        }
        Ok(step)
    }
    fn weight(&mut self) -> Result<Ratio, String> {
        let start = self.pos;
        let whole = self.integer()?;
        let mut num = u128::from(whole);
        let mut den: u128 = 1;
        if self.peek() == Some(b'.') {
            self.pos += 1;
            let digits = self.pos;
            while self.peek().is_some_and(|c| c.is_ascii_digit()) {
                let c = self.peek().unwrap_or(b'0');
                num = num
                    .checked_mul(10)
                    .and_then(|n| n.checked_add(u128::from(c - b'0')))
                    .ok_or_else(|| format!("at byte {start}: weight has too many digits"))?;
                den = den
                    .checked_mul(10)
                    .ok_or_else(|| format!("at byte {start}: weight has too many digits"))?;
                self.pos += 1;
            }
            if self.pos == digits {
                return self.fail("expected digits after '.'");
            }
        }
        if num == 0 {
            return Err(format!("at byte {start}: weight must be positive"));
        }
        if u64::try_from(num).is_err() || u64::try_from(den).is_err() {
            return Err(format!("at byte {start}: weight is too large"));
        }
        Ok(Ratio::reduce(num, den))
    }
    fn euclid(&mut self) -> Result<Euclid, String> {
        self.skip_ws();
        let pulses = self.integer()?;
        self.expect(b',')?;
        self.skip_ws();
        let steps = self.integer()?;
        self.skip_ws();
        let rotate = if self.peek() == Some(b',') {
            self.pos += 1;
            self.skip_ws();
            self.integer()?
        } else {
            0
        };
        self.expect(b')')?;
        if steps == 0 || steps > MAX_STEPS || pulses > steps {
            return self.fail("expected euclid 1 <= pulses <= steps <= 1024");
        }
        if rotate > MAX_STEPS {
            return self.fail("expected euclid rotation 0..=1024");
        }
        Ok(Euclid {
            pulses,
            steps,
            rotate: rotate as i64,
        })
    }
    fn atom(&mut self, depth: usize) -> Result<Atom, String> {
        let Some(c) = self.peek() else {
            return self.fail("expected a step, found end of pattern");
        };
        match c {
            b'~' => {
                self.pos += 1;
                Ok(Atom::Rest)
            }
            b'[' => {
                self.enter(depth)?;
                self.pos += 1;
                let inner = self.stack(depth + 1)?;
                self.expect(b']')?;
                Ok(Atom::Stack(inner))
            }
            b'<' => {
                self.enter(depth)?;
                self.pos += 1;
                let inner = self.sequence(depth + 1)?;
                self.expect(b'>')?;
                Ok(Atom::Alt(inner))
            }
            b'{' => {
                self.enter(depth)?;
                self.pos += 1;
                let inner = self.sequence(depth + 1)?;
                self.expect(b'}')?;
                self.expect(b'%')?;
                let per_cycle = self.integer()?;
                if per_cycle == 0 || per_cycle > MAX_STEPS {
                    return self.fail("expected a '%' step count 1..=1024");
                }
                Ok(Atom::Poly {
                    steps: inner,
                    per_cycle,
                })
            }
            b'-' | b'0'..=b'9' => self.number(),
            b'a'..=b'z' | b'_' => self.word(),
            _ => self.fail(&format!("unexpected {}", self.here())),
        }
    }
    fn enter(&self, depth: usize) -> Result<(), String> {
        if depth + 1 > MAX_DEPTH {
            return self.fail("nesting deeper than 8 levels");
        }
        Ok(())
    }
    fn number(&mut self) -> Result<Atom, String> {
        let start = self.pos;
        if self.peek() == Some(b'-') {
            self.pos += 1;
        }
        let digits = self.pos;
        while self.peek().is_some_and(|c| c.is_ascii_digit()) {
            self.pos += 1;
        }
        if self.pos == digits {
            return self.fail("expected digits");
        }
        if self.peek() == Some(b'.') {
            self.pos += 1;
            let frac = self.pos;
            while self.peek().is_some_and(|c| c.is_ascii_digit()) {
                self.pos += 1;
            }
            if self.pos == frac {
                return self.fail("expected digits after '.'");
            }
        }
        let text = &self.src[start..self.pos];
        let value: f64 = text
            .parse()
            .map_err(|_| format!("at byte {start}: {text} is not a number"))?;
        Ok(Atom::Token(Token::Number(value)))
    }
    fn word(&mut self) -> Result<Atom, String> {
        if let Some((end, midi)) = self.note_at() {
            let Some(midi) = u8::try_from(midi).ok().filter(|&m| m <= 127) else {
                let name = &self.src[self.pos..end];
                return self.fail(&format!("note {name} is outside MIDI 0..=127"));
            };
            let name = self.src[self.pos..end].to_string();
            self.pos = end;
            return Ok(Atom::Token(Token::Note { name, midi }));
        }
        let start = self.pos;
        while self
            .peek()
            .is_some_and(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'_')
        {
            self.pos += 1;
        }
        let word = &self.src[start..self.pos];
        if word == "x" {
            return Ok(Atom::Token(Token::Hit));
        }
        if word.bytes().all(|c| c == b'_') {
            return Err(format!(
                "at byte {start}: '_' is not a token (elongate is not in this grammar)"
            ));
        }
        Ok(Atom::Token(Token::Name(word.to_string())))
    }
    /// `[a-g](#|b)?-?\d` ending the word: the end offset and the MIDI number (c1 = 36).
    fn note_at(&self) -> Option<(usize, i64)> {
        let b = self.src.as_bytes();
        let mut i = self.pos;
        let letter = *b.get(i)?;
        let mut pc: i64 = match letter {
            b'c' => 0,
            b'd' => 2,
            b'e' => 4,
            b'f' => 5,
            b'g' => 7,
            b'a' => 9,
            b'b' => 11,
            _ => return None,
        };
        i += 1;
        match b.get(i) {
            Some(b'#') => {
                pc += 1;
                i += 1;
            }
            Some(b'b') => {
                pc -= 1;
                i += 1;
            }
            _ => {}
        }
        let negative = b.get(i) == Some(&b'-');
        if negative {
            i += 1;
        }
        let digit = *b.get(i)?;
        if !digit.is_ascii_digit() {
            return None;
        }
        i += 1;
        if !ends_token(b.get(i).copied()) {
            return None;
        }
        let octave = i64::from(digit - b'0') * if negative { -1 } else { 1 };
        Some((i, (octave + 2) * 12 + pc))
    }
}
fn ends_step(c: Option<u8>) -> bool {
    match c {
        None | Some(b']' | b'>' | b'}' | b',') => true,
        Some(w) => w.is_ascii_whitespace(),
    }
}
fn ends_token(c: Option<u8>) -> bool {
    matches!(c, Some(b'*' | b'@' | b'?' | b'(')) || ends_step(c)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::music::rhythm::Expression;

    fn r(num: u64, den: u64) -> Ratio {
        Ratio::new(num, den)
    }
    fn parse(text: &str) -> Pattern {
        Pattern::parse(text).unwrap_or_else(|e| panic!("{text}: {e}"))
    }
    fn onsets(text: &str, k: u64) -> Vec<Ratio> {
        parse(text).cycle(k).into_iter().map(|e| e.onset).collect()
    }
    /// Onset, span, ratchet, draw for each event of one cycle.
    fn shape(text: &str, k: u64) -> Vec<(Ratio, Ratio, u8, Draw)> {
        parse(text)
            .cycle(k)
            .into_iter()
            .map(|e| (e.onset, e.span, e.ratchet, e.draw))
            .collect()
    }
    fn sixteenths(slots: &[u64]) -> Vec<Ratio> {
        slots.iter().map(|&j| r(j, 16)).collect()
    }

    #[test]
    fn ratio_reduces_orders_and_floors() {
        assert_eq!(r(2, 8), r(1, 4));
        assert_eq!(r(0, 7), Ratio::ZERO);
        assert!(r(1, 3) < r(1, 2) && r(7, 8) > r(3, 4));
        assert_eq!(r(3, 4) + r(1, 8), r(7, 8));
        assert_eq!(r(1, 2) * r(1, 4), r(1, 8));
        assert_eq!(r(1, 16).mul_int(3), r(3, 16));
        assert_eq!(r(7, 8).ticks(3840), 3360);
        // floor, and no overflow on the way there
        assert_eq!(r(1, 7).ticks(3840), 548);
        assert_eq!(r(1, 3).ticks(u64::MAX), u64::MAX / 3);
    }

    #[test]
    fn worked_example_hits_with_a_subdivided_last_step() {
        // x x x [x x?] -> 0, 1/4, 2/4, 3/4, 7/8 (draw gates the last)
        assert_eq!(
            shape("x x x [x x?]", 0),
            vec![
                (Ratio::ZERO, r(1, 4), 1, Draw::None),
                (r(1, 4), r(1, 4), 1, Draw::None),
                (r(1, 2), r(1, 4), 1, Draw::None),
                (r(3, 4), r(1, 8), 1, Draw::None),
                (r(7, 8), r(1, 8), 1, Draw::Hit),
            ]
        );
    }

    #[test]
    fn worked_example_rests_inside_subdivisions() {
        // x ~ [~ x] [~ x?] -> 0, 5/8, 7/8 (draw gates the last)
        assert_eq!(
            shape("x ~ [~ x] [~ x?]", 0),
            vec![
                (Ratio::ZERO, r(1, 4), 1, Draw::None),
                (r(5, 8), r(1, 8), 1, Draw::None),
                (r(7, 8), r(1, 8), 1, Draw::Hit),
            ]
        );
    }

    #[test]
    fn worked_example_alternation_changes_the_last_step() {
        // ~ x ~ <x [x x*2]> -> cycle 0: 1/4, 3/4 · cycle 1: 1/4, 3/4, 7/8 ratchet 2 over 1/8
        assert_eq!(
            shape("~ x ~ <x [x x*2]>", 0),
            vec![
                (r(1, 4), r(1, 4), 1, Draw::None),
                (r(3, 4), r(1, 4), 1, Draw::None),
            ]
        );
        assert_eq!(
            shape("~ x ~ <x [x x*2]>", 1),
            vec![
                (r(1, 4), r(1, 4), 1, Draw::None),
                (r(3, 4), r(1, 8), 1, Draw::None),
                (r(7, 8), r(1, 8), 2, Draw::None),
            ]
        );
        // the tail of that ratchet lands at 15/16
        let tail = r(7, 8) + r(1, 8) * r(1, 2);
        assert_eq!(tail, r(15, 16));
        assert_eq!(shape("~ x ~ <x [x x*2]>", 2), shape("~ x ~ <x [x x*2]>", 0));
    }

    #[test]
    fn worked_example_draw_gates_a_ratchet_tail() {
        // ~ x ~ [x x*3?] -> 1/4, 3/4, 7/8 ratchet 3 over 1/8, draw on the tail
        assert_eq!(
            shape("~ x ~ [x x*3?]", 0),
            vec![
                (r(1, 4), r(1, 4), 1, Draw::None),
                (r(3, 4), r(1, 8), 1, Draw::None),
                (r(7, 8), r(1, 8), 3, Draw::Tail),
            ]
        );
    }

    #[test]
    fn worked_example_weight() {
        // x@3 [~ x] -> 0 (span 3/4), 7/8
        assert_eq!(
            shape("x@3 [~ x]", 0),
            vec![
                (Ratio::ZERO, r(3, 4), 1, Draw::None),
                (r(7, 8), r(1, 8), 1, Draw::None),
            ]
        );
        // a fractional weight is exact: 1.5 : 1 : 1 shares of the cycle
        assert_eq!(onsets("x@1.5 x x", 0), vec![Ratio::ZERO, r(3, 7), r(5, 7)]);
    }

    #[test]
    fn worked_example_polymeter_walks_across_the_cycle_line() {
        // {x ~ x x ~ x ~}%16, m = 7, n = 16: slot j of cycle k plays element (16k + j) mod 7.
        // Elements 0,2,3,5 are hits; 1,4,6 are rests.
        let p = "{x ~ x x ~ x ~}%16";
        // cycle 0: element index = j mod 7 -> hits where (j mod 7) in {0,2,3,5}
        assert_eq!(onsets(p, 0), sixteenths(&[0, 2, 3, 5, 7, 9, 10, 12, 14]));
        // cycle 1: 16 mod 7 = 2, index = (j + 2) mod 7
        assert_eq!(
            onsets(p, 1),
            sixteenths(&[0, 1, 3, 5, 7, 8, 10, 12, 14, 15])
        );
        // cycle 6: 96 mod 7 = 5, index = (j + 5) mod 7
        assert_eq!(onsets(p, 6), sixteenths(&[0, 2, 4, 5, 7, 9, 11, 12, 14]));
        // every slot is a sixteenth of the cycle
        assert!(parse(p).cycle(0).iter().all(|e| e.span == r(1, 16)));
        // and it comes home after lcm(7,16)/16 = 7 cycles
        assert_eq!(parse(p).period_cycles(), Some(7));
        assert_eq!(onsets(p, 7), onsets(p, 0));
    }

    #[test]
    fn worked_example_euclid() {
        // x(5,8) -> 0, 2/8, 4/8, 5/8, 7/8 by (i*5) mod 8 < 5
        assert_eq!(
            shape("x(5,8)", 0),
            vec![
                (Ratio::ZERO, r(1, 8), 1, Draw::None),
                (r(1, 4), r(1, 8), 1, Draw::None),
                (r(1, 2), r(1, 8), 1, Draw::None),
                (r(5, 8), r(1, 8), 1, Draw::None),
                (r(7, 8), r(1, 8), 1, Draw::None),
            ]
        );
    }

    #[test]
    fn euclid_matches_the_euclidean_leaf() {
        for (pulses, steps, rotation) in [(5, 8, 0), (3, 8, 0), (3, 8, 2), (7, 16, 3), (1, 4, 1)] {
            let leaf = Expression::Euclidean {
                steps,
                pulses,
                rotation,
                reset_on_phrase: false,
            };
            let expected: Vec<Ratio> = (0..u64::from(steps))
                .filter(|&i| leaf.evaluate_position(i, i, &|_, _| false).active())
                .map(|i| r(i, u64::from(steps)))
                .collect();
            let text = format!("x({pulses},{steps},{rotation})");
            assert_eq!(onsets(&text, 0), expected, "{text}");
        }
    }

    #[test]
    fn worked_example_chord_is_five_simultaneous_values() {
        let events = parse("[1,3,5,7,9]").cycle(0);
        assert_eq!(events.len(), 5);
        assert!(
            events
                .iter()
                .all(|e| e.onset == Ratio::ZERO && e.span == Ratio::ONE)
        );
        // stacked events keep source order at equal onsets
        let values: Vec<f64> = events
            .iter()
            .map(|e| match e.token {
                Token::Number(v) => v,
                _ => panic!("expected numbers"),
            })
            .collect();
        assert_eq!(values, vec![1.0, 3.0, 5.0, 7.0, 9.0]);
        assert_eq!(
            parse("[1,3,5,7,9]").kinds(),
            TokenKinds {
                numbers: true,
                ..TokenKinds::default()
            }
        );
    }

    #[test]
    fn worked_example_note_alternation() {
        let p = parse("<c1 c1 eb1 bb0>");
        let midi: Vec<u8> = (0..5)
            .map(|k| match &p.cycle(k)[0].token {
                Token::Note { midi, .. } => *midi,
                other => panic!("expected a note, got {other:?}"),
            })
            .collect();
        assert_eq!(midi, vec![36, 36, 39, 34, 36]);
        assert_eq!(p.period_cycles(), Some(4));
        match &p.cycle(2)[0].token {
            Token::Note { name, .. } => assert_eq!(name, "eb1"),
            other => panic!("expected a note, got {other:?}"),
        }
    }

    #[test]
    fn note_names_span_accidentals_and_negative_octaves() {
        let cases = [
            ("c1", 36u8),
            ("eb1", 39),
            ("bb0", 34),
            ("f#-1", 18),
            ("c0", 24),
        ];
        for (text, expect) in cases {
            match &parse(text).cycle(0)[0].token {
                Token::Note { midi, name } => {
                    assert_eq!((*midi, name.as_str()), (expect, text), "{text}");
                }
                other => panic!("{text}: expected a note, got {other:?}"),
            }
        }
        // a word that is not note-shaped is a name
        assert!(matches!(parse("rifle").cycle(0)[0].token, Token::Name(_)));
        // out of MIDI range is an error, not a silent name
        assert!(Pattern::parse("c9").is_err());
    }

    #[test]
    fn worked_example_slice_row_leaves_the_rests_out() {
        let p = "1 2 3 ~ 5 ~ 7 8 1 ~ 3 4 ~ 6 7 ~";
        let got: Vec<u64> = parse(p)
            .cycle(0)
            .iter()
            .map(|e| e.onset.num * 16 / e.onset.den)
            .collect();
        // rests at 3, 5, 9, 12, 15
        assert_eq!(got, vec![0, 1, 2, 4, 6, 7, 8, 10, 11, 13, 14]);
        assert_eq!(parse(p).cycle(0).len(), 11);
    }

    #[test]
    fn worked_example_lone_ratchet() {
        // ~ 12*6 -> one event at 1/2, token 12, ratchet 6 over span 1/2
        let events = parse("~ 12*6").cycle(0);
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].onset, r(1, 2));
        assert_eq!(events[0].span, r(1, 2));
        assert_eq!(events[0].ratchet, 6);
        assert_eq!(events[0].draw, Draw::None);
        assert_eq!(events[0].token, Token::Number(12.0));
    }

    #[test]
    fn ticks_place_plain_hits_and_a_draw() {
        let got: Vec<(u64, Draw)> = parse("x x x [x x?]")
            .ticks(0, 3840)
            .into_iter()
            .map(|e| (e.tick, e.draw))
            .collect();
        assert_eq!(
            got,
            vec![
                (0, Draw::None),
                (960, Draw::None),
                (1920, Draw::None),
                (2880, Draw::None),
                (3360, Draw::Hit),
            ]
        );
    }

    #[test]
    fn ticks_place_ratchet_tails() {
        let got = parse("~ x ~ [x x*3?]").ticks(0, 3840);
        assert_eq!(
            got.iter().map(|e| e.tick).collect::<Vec<_>>(),
            vec![960, 2880, 3360]
        );
        assert_eq!(got[2].span_ticks, 480);
        assert_eq!(got[2].tails, vec![3520, 3680]);
        assert_eq!(got[2].draw, Draw::Tail);
        assert!(got[0].tails.is_empty() && got[1].tails.is_empty());
    }

    #[test]
    fn ticks_place_a_six_way_ratchet() {
        let got = parse("~ 12*6").ticks(0, 3840);
        assert_eq!(got.len(), 1);
        assert_eq!((got[0].tick, got[0].span_ticks), (1920, 1920));
        let expected: Vec<u64> = (1..6).map(|i| 1920 + 1920 * i / 6).collect();
        assert_eq!(got[0].tails, expected);
        assert_eq!(got[0].tails, vec![2240, 2560, 2880, 3200, 3520]);
    }

    #[test]
    fn ticks_offset_by_the_cycle_index() {
        let got = parse("x x x [x x?]").ticks(2, 3840);
        assert_eq!(
            got.iter().map(|e| e.tick).collect::<Vec<_>>(),
            vec![7680, 8640, 9600, 10560, 11040]
        );
    }

    #[test]
    fn ticks_floor_a_seven_tuplet() {
        let got = parse("x x x x x x x").ticks(0, 3840);
        assert_eq!(
            got.iter().map(|e| e.tick).collect::<Vec<_>>(),
            vec![0, 548, 1097, 1645, 2194, 2742, 3291]
        );
        // spans tile the cycle exactly despite the floor
        assert_eq!(got.iter().map(|e| e.span_ticks).sum::<u64>(), 3840);
    }

    #[test]
    fn period_cycles_is_the_lcm_of_every_alternation_and_polymeter() {
        assert_eq!(parse("x ~ x").period_cycles(), Some(1));
        assert_eq!(parse("<x ~ x>").period_cycles(), Some(3));
        assert_eq!(parse("<x ~>").period_cycles(), Some(2));
        assert_eq!(parse("{1 2 3 4 5 6 7}%16").period_cycles(), Some(7));
        // lcm(3, 2) = 6
        assert_eq!(parse("<x ~ x> <x ~>").period_cycles(), Some(6));
        // lcm(3, 7) = 21, alternation nested in a subdivision still counts
        assert_eq!(
            parse("[<x ~ x> x] {1 2 3 4 5 6 7}%16").period_cycles(),
            Some(21)
        );
        // an alternation inside an alternation still reads cycle k
        assert_eq!(parse("<x <x ~ x>>").period_cycles(), Some(6));
    }

    #[test]
    fn nested_alternation_uses_the_cycle_index_directly() {
        let p = parse("<x <a b c>>");
        // odd cycles pick the inner alternation, which itself reads k mod 3 — not a slowed k
        assert_eq!(p.cycle(0)[0].token, Token::Hit);
        assert!(matches!(&p.cycle(1)[0].token, Token::Name(n) if n == "b"));
        assert!(matches!(&p.cycle(3)[0].token, Token::Name(n) if n == "a"));
        assert!(matches!(&p.cycle(5)[0].token, Token::Name(n) if n == "c"));
    }

    #[test]
    fn kinds_report_every_token_family() {
        assert_eq!(
            parse("x ~ x").kinds(),
            TokenKinds {
                hits: true,
                ..TokenKinds::default()
            }
        );
        assert_eq!(
            parse("rifle ~ pistol pistol").kinds(),
            TokenKinds {
                names: true,
                ..TokenKinds::default()
            }
        );
        assert_eq!(
            parse("<c1 eb1>").kinds(),
            TokenKinds {
                notes: true,
                ..TokenKinds::default()
            }
        );
        assert_eq!(
            parse("x 1 c1 rifle").kinds(),
            TokenKinds {
                hits: true,
                numbers: true,
                notes: true,
                names: true,
            }
        );
        assert_eq!(parse("~ ~").kinds(), TokenKinds::default());
    }

    #[test]
    fn rejects_everything_the_spec_leaves_out() {
        let rejected = [
            "x/2",        // slow
            "x / 2",      //
            "x!",         // replicate
            "x!2",        //
            "x ! x",      //
            "x . x",      // grouping
            "x.every(2)", // whole-pattern operator
            "x _",        // elongate
            "_",          //
            "x | ~",      // random choice
            "x?0.3",      // explicit probability
            "{x ~ x}",    // polymetric step without %
            "{x ~ x}%",   //
        ];
        for text in rejected {
            assert!(
                Pattern::parse(text).is_err(),
                "{text} should not have parsed"
            );
        }
    }

    #[test]
    fn rejects_malformed_patterns() {
        let rejected = [
            "",
            "   ",
            ",",
            "x ,",
            "[]",
            "[x",
            "<>",
            "<x",
            "x]",
            "x**2",
            "x*2*3",
            "x*1",
            "x*9",
            "x@",
            "x@0",
            "x@-2",
            "x??",
            "x@2@3",
            "x(5)",
            "x(9,8)",
            "x(5,0)",
            "x(5,8",
            "x(5,8)(3,4)",
            "{x x}%0",
            "$",
            "X",
            "c9",
            "1.",
            ".5",
            "-",
            "x 2.3.4",
        ];
        for text in rejected {
            assert!(
                Pattern::parse(text).is_err(),
                "{text:?} should not have parsed"
            );
        }
    }

    #[test]
    fn errors_name_the_offset_and_what_was_expected() {
        let err = Pattern::parse("x x | x").unwrap_err();
        assert!(err.starts_with("at byte 4:"), "{err}");
        assert!(err.contains("unexpected"), "{err}");
        let err = Pattern::parse("x*9").unwrap_err();
        assert!(err.contains("at byte 3") && err.contains("2..=8"), "{err}");
    }

    #[test]
    fn nesting_is_capped_at_eight() {
        let ok = format!("{}x{}", "[".repeat(8), "]".repeat(8));
        assert!(Pattern::parse(&ok).is_ok());
        let deep = format!("{}x{}", "[".repeat(9), "]".repeat(9));
        assert!(Pattern::parse(&deep).is_err());
    }

    #[test]
    fn a_one_element_alternation_is_that_element() {
        assert_eq!(shape("<x>", 0), shape("x", 0));
        assert_eq!(shape("<x>", 5), shape("x", 5));
    }

    #[test]
    fn a_stack_sorts_by_onset_and_keeps_source_order() {
        // two sequences over the same span; the pair at onset 0 keeps the written order
        let events = parse("x@3 x, x x").cycle(0);
        assert_eq!(
            events.iter().map(|e| (e.onset, e.span)).collect::<Vec<_>>(),
            vec![
                (Ratio::ZERO, r(3, 4)),
                (Ratio::ZERO, r(1, 2)),
                (r(1, 2), r(1, 2)),
                (r(3, 4), r(1, 4)),
            ]
        );
    }

    #[test]
    fn modifiers_reach_the_leaves_of_a_composite_step() {
        // a ratchet on a group applies to each leaf over that leaf's own span; an inner
        // ratchet wins over an inherited one.
        let events = parse("[x x*4]*2").cycle(0);
        assert_eq!(
            events
                .iter()
                .map(|e| (e.onset, e.ratchet))
                .collect::<Vec<_>>(),
            vec![(Ratio::ZERO, 2), (r(1, 2), 4)]
        );
    }
}

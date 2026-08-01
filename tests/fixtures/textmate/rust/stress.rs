#![allow(dead_code)]
#![allow(clippy::needless_lifetimes)]

//! Stress fixture with non-ASCII text: café λ🚀.
//! It models a tiny telemetry protocol for stations from Zürich to 東京.

use std::{
    borrow::Cow,
    collections::{BTreeMap, VecDeque},
    ffi::CStr,
    fmt::{self, Display},
    future::Future,
    marker::PhantomData,
    ops::RangeInclusive,
};

/* outer block comment starts
   /* nested block comment exercises recursive begin/end rules */
   still inside the outer comment
*/

/// Describes input while preserving escaped and raw-string forms.
pub fn describe<'a>(input: &'a str) -> Result<String, &'static str> {
    let raw = r###"raw string with "quotes", hashes ##, and a newline
second raw line with emoji 🚀 and lambda λ"###;
    let escaped = "line one\nline two with café and an escaped quote: \"";
    let formatted = format!("{input:?} => {raw}\n{escaped}");
    Ok(formatted)
}

macro_rules! nested_tokens {
    ($name:ident, $value:expr) => {
        pub const $name: &str = stringify!($value);
    };
}

nested_tokens!(UNICODE_NAME, café_λ);

const MAGIC: u32 = 0xCAFE_BABE;
const FLAGS: u8 = 0b1010_0110;
const MODE: u16 = 0o755;
const RAW_PACKET: &[u8] = br##"GET /v1/measurements?tag="#rust""##;
const C_GREETING: &CStr = c"Grüße from station 🚀";
const RAW_C_PATH: &CStr = cr#"C:\telemetry\new"#;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParseIssue {
    pub offset: usize,
    pub message: Cow<'static, str>,
}

impl Display for ParseIssue {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "byte {}: {}", self.offset, self.message)
    }
}

#[derive(Clone, Debug)]
#[repr(C)]
pub struct Envelope<'a, T, const N: usize>
where
    T: Clone,
{
    pub id: u64,
    pub label: Cow<'a, str>,
    pub payload: [T; N],
    marker: PhantomData<&'a T>,
}

impl<'a, T, const N: usize> Envelope<'a, T, N>
where
    T: Clone + Default,
{
    pub fn empty(id: u64, label: impl Into<Cow<'a, str>>) -> Self {
        Self {
            id,
            label: label.into(),
            payload: std::array::from_fn(|_| T::default()),
            marker: PhantomData,
        }
    }

    pub fn first(&self) -> Option<&T> {
        self.payload.first()
    }
}

#[derive(Clone, Debug)]
pub enum Event<'a, T> {
    Started { station: &'a str, at: u64 },
    Samples(&'a [T]),
    Message(Cow<'a, str>),
    Finished(Result<T, ParseIssue>),
}

/// Produces a human-readable summary tied to the borrow lifetime.
pub trait Summarize<'a> {
    type Output: Display + 'a;
    const KIND: &'static str;

    fn summarize(&'a self) -> Self::Output;

    fn kind(&self) -> &'static str {
        Self::KIND
    }
}

impl<'a, T, const N: usize> Summarize<'a> for Envelope<'a, T, N>
where
    T: Clone + Display,
{
    type Output = String;
    const KIND: &'static str = "envelope";

    fn summarize(&'a self) -> Self::Output {
        let values = self.payload.iter().map(ToString::to_string).collect::<Vec<_>>();
        format!("{}#{} [{}]", self.label, self.id, values.join(", "))
    }
}

pub mod protocol {
    use super::{Cow, Event, ParseIssue, RangeInclusive};

    pub const SUPPORTED: RangeInclusive<u8> = 1..=3;

    pub fn classify<'a>(event: &'a Event<'a, i32>) -> Cow<'a, str> {
        match event {
            Event::Started { station: "東京", at: 0..=86_400 } => Cow::Borrowed("daily start"),
            Event::Started { station, at } if *at > 86_400 => {
                Cow::Owned(format!("late {station} at {at}"))
            }
            Event::Samples([first, middle @ .., last]) => {
                Cow::Owned(format!("{first} + {} + {last}", middle.len()))
            }
            Event::Samples([]) => Cow::Borrowed("empty"),
            Event::Samples([single]) => Cow::Owned(format!("single {single}")),
            Event::Message(text) => text.clone(),
            Event::Finished(Ok(value)) => Cow::Owned(format!("ok:{value}")),
            Event::Finished(Err(ParseIssue { offset, .. })) => Cow::Owned(format!("error@{offset}")),
            Event::Started { .. } => Cow::Borrowed("start"),
        }
    }
}

macro_rules! ordered_map {
    ($($key:expr => $value:expr),+ $(,)?) => {{
        let mut map = BTreeMap::new();
        $(map.insert($key, $value);)+
        map
    }};
}

pub fn collect_metrics(limit: u8) -> BTreeMap<&'static str, f64> {
    let scale = 1.25_f64;
    let mut queue: VecDeque<u32> = (0..=limit).map(u32::from).collect();
    let mut sum = 0_u64;
    while let Some(front @ 0..=9) = queue.pop_front() {
        sum += u64::from(front);
    }
    let normalize = move |value: u64| -> f64 { value as f64 / scale };
    ordered_map! {
        "sum" => normalize(sum),
        "π-adjusted" => std::f64::consts::PI * 1e-3,
        "infinity" => f64::INFINITY,
    }
}

pub fn select_max<'a, T>(left: &'a T, right: &'a T) -> &'a T
where
    T: PartialOrd + ?Sized,
{
    if left >= right { left } else { right }
}

pub async fn load_station<F, Fut>(name: &str, fetch: F) -> Result<String, ParseIssue>
where
    F: FnOnce(String) -> Fut,
    Fut: Future<Output = Result<Vec<u8>, ParseIssue>>,
{
    let path = format!("stations/{name}/latest");
    let bytes = fetch(path).await?;
    let text = String::from_utf8_lossy(&bytes);
    Ok(text.trim_matches(|character: char| character.is_whitespace()).to_owned())
}

#[inline]
pub fn syntax_sampler(tuple: (i32, i32), bytes: &[u8]) -> usize {
    let (x, y) = tuple;
    let sign = match x.cmp(&y) {
        std::cmp::Ordering::Less => -1_i8,
        std::cmp::Ordering::Equal => 0,
        std::cmp::Ordering::Greater => 1,
    };
    let r#type = "sensor";
    let glyphs = ['é', 'λ', '水', '🦀'];
    let printable = bytes.iter().filter(|byte| matches!(byte, b' '..=b'~')).count();
    assert!(matches!(sign, -1..=1), "invalid sign for {type}");
    printable + glyphs.len() + MAGIC.count_ones() as usize + FLAGS as usize + MODE as usize
}

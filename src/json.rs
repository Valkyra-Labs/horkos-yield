//! A small JSON reader and writer for the WebAssembly boundary, so the
//! crate needs no serialisation dependency.
//!
//! Numbers that JSON cannot hold are written as the strings `"NaN"`,
//! `"Infinity"` and `"-Infinity"`; on input, `null` or a missing number is
//! NaN (what `JSON.stringify` makes of NaN).

use crate::{
    Amortization, Breakdown, Calculation, CouponType, Derived, Error, Issue, Market, Plan,
    Schedule, TaxRegime,
};
use std::fmt::Write;

#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Null,
    Bool(bool),
    Num(f64),
    Str(String),
    Arr(Vec<Value>),
    Obj(Vec<(String, Value)>),
}

static NULL: Value = Value::Null;

impl Value {
    pub fn get(&self, key: &str) -> &Value {
        match self {
            Value::Obj(kv) => kv.iter().find(|(k, _)| k == key).map_or(&NULL, |(_, v)| v),
            _ => &NULL,
        }
    }

    fn num(&self) -> f64 {
        match self {
            Value::Num(x) => *x,
            Value::Str(s) => match s.as_str() {
                "NaN" => f64::NAN,
                "Infinity" => f64::INFINITY,
                "-Infinity" => f64::NEG_INFINITY,
                _ => f64::NAN,
            },
            _ => f64::NAN,
        }
    }

    fn str(&self) -> &str {
        match self {
            Value::Str(s) => s,
            _ => "",
        }
    }

    fn arr(&self) -> &[Value] {
        match self {
            Value::Arr(v) => v,
            _ => &[],
        }
    }
}

struct Parser<'a> {
    b: &'a [u8],
    i: usize,
}

/// Parses one JSON document; `None` when it is not valid JSON.
pub fn parse(s: &str) -> Option<Value> {
    let mut p = Parser {
        b: s.as_bytes(),
        i: 0,
    };
    let v = p.value(0)?;
    p.ws();
    (p.i == p.b.len()).then_some(v)
}

impl Parser<'_> {
    fn ws(&mut self) {
        while let Some(c) = self.b.get(self.i) {
            if matches!(c, b' ' | b'\t' | b'\n' | b'\r') {
                self.i += 1;
            } else {
                break;
            }
        }
    }

    fn eat(&mut self, lit: &[u8]) -> Option<()> {
        if self.b.get(self.i..self.i + lit.len())? == lit {
            self.i += lit.len();
            Some(())
        } else {
            None
        }
    }

    fn value(&mut self, depth: usize) -> Option<Value> {
        if depth > 64 {
            return None;
        }
        self.ws();
        match *self.b.get(self.i)? {
            b'n' => self.eat(b"null").map(|_| Value::Null),
            b't' => self.eat(b"true").map(|_| Value::Bool(true)),
            b'f' => self.eat(b"false").map(|_| Value::Bool(false)),
            b'"' => self.string().map(Value::Str),
            b'[' => {
                self.i += 1;
                let mut out = Vec::new();
                self.ws();
                if self.eat(b"]").is_some() {
                    return Some(Value::Arr(out));
                }
                loop {
                    out.push(self.value(depth + 1)?);
                    self.ws();
                    if self.eat(b",").is_none() {
                        self.eat(b"]")?;
                        return Some(Value::Arr(out));
                    }
                }
            }
            b'{' => {
                self.i += 1;
                let mut out = Vec::new();
                self.ws();
                if self.eat(b"}").is_some() {
                    return Some(Value::Obj(out));
                }
                loop {
                    self.ws();
                    let k = self.string()?;
                    self.ws();
                    self.eat(b":")?;
                    out.push((k, self.value(depth + 1)?));
                    self.ws();
                    if self.eat(b",").is_none() {
                        self.eat(b"}")?;
                        return Some(Value::Obj(out));
                    }
                }
            }
            _ => self.number(),
        }
    }

    fn number(&mut self) -> Option<Value> {
        let start = self.i;
        while let Some(c) = self.b.get(self.i) {
            if c.is_ascii_digit() || matches!(c, b'-' | b'+' | b'.' | b'e' | b'E') {
                self.i += 1;
            } else {
                break;
            }
        }
        let s = std::str::from_utf8(&self.b[start..self.i]).ok()?;
        // Rust accepts forms JSON does not ("inf", "1."); the scan above
        // already excludes letters other than e.
        if s.is_empty() || s.ends_with('.') || s.starts_with('.') || s.starts_with("-.") {
            return None;
        }
        s.parse::<f64>().ok().map(Value::Num)
    }

    fn hex4(&mut self) -> Option<u32> {
        let s = std::str::from_utf8(self.b.get(self.i..self.i + 4)?).ok()?;
        self.i += 4;
        u32::from_str_radix(s, 16).ok()
    }

    fn string(&mut self) -> Option<String> {
        self.eat(b"\"")?;
        let mut out = String::new();
        loop {
            let start = self.i;
            while let Some(&c) = self.b.get(self.i) {
                if c == b'"' || c == b'\\' || c < 0x20 {
                    break;
                }
                self.i += 1;
            }
            out.push_str(std::str::from_utf8(&self.b[start..self.i]).ok()?);
            match *self.b.get(self.i)? {
                b'"' => {
                    self.i += 1;
                    return Some(out);
                }
                b'\\' => {
                    self.i += 1;
                    let c = *self.b.get(self.i)?;
                    self.i += 1;
                    match c {
                        b'"' => out.push('"'),
                        b'\\' => out.push('\\'),
                        b'/' => out.push('/'),
                        b'b' => out.push('\u{8}'),
                        b'f' => out.push('\u{c}'),
                        b'n' => out.push('\n'),
                        b'r' => out.push('\r'),
                        b't' => out.push('\t'),
                        b'u' => {
                            let hi = self.hex4()?;
                            let cp = if (0xd800..0xdc00).contains(&hi) {
                                self.eat(b"\\u")?;
                                let lo = self.hex4()?;
                                if !(0xdc00..0xe000).contains(&lo) {
                                    return None;
                                }
                                0x10000 + ((hi - 0xd800) << 10) + (lo - 0xdc00)
                            } else {
                                hi
                            };
                            out.push(char::from_u32(cp)?);
                        }
                        _ => return None,
                    }
                }
                _ => return None,
            }
        }
    }
}

fn opt_num(v: &Value) -> Option<f64> {
    match v {
        Value::Null => None,
        v => Some(v.num()),
    }
}

/// An issue from its JSON form; an unknown coupon type is
/// [`Error::InvalidCode`].
pub fn issue_from(v: &Value) -> Result<Issue, Error> {
    let coupon_type = CouponType::from_code(v.get("couponType").str()).ok_or(Error::InvalidCode)?;
    Ok(Issue {
        nominal: v.get("nominal").num(),
        price_pct: v.get("pricePct").num(),
        accrued: opt_num(v.get("accrued")),
        coupon_type,
        coupon_rate_pct: v.get("couponRatePct").num(),
        spread_pct: v.get("spreadPct").num(),
        period_days: v.get("periodDays").num(),
        maturity: v.get("maturity").str().to_owned(),
        offers: v
            .get("offers")
            .arr()
            .iter()
            .map(|o| o.str().to_owned())
            .collect(),
        amortization: v
            .get("amortization")
            .arr()
            .iter()
            .map(|a| Amortization {
                date: a.get("date").str().to_owned(),
                fraction_pct: a.get("fractionPct").num(),
            })
            .collect(),
    })
}

pub fn market_from(v: &Value) -> Market {
    Market {
        valuation_date: v.get("valuationDate").str().to_owned(),
        key_rate_pct: v.get("keyRatePct").num(),
    }
}

/// A plan from its JSON form; an unknown tax regime is
/// [`Error::InvalidCode`].
pub fn plan_from(v: &Value) -> Result<Plan, Error> {
    let tax_regime = TaxRegime::from_code(v.get("taxRegime").str()).ok_or(Error::InvalidCode)?;
    Ok(Plan {
        amount: v.get("amount").num(),
        horizon_day: v.get("horizonDay").num(),
        reinvest: matches!(v.get("reinvest"), Value::Bool(true)),
        tax_regime,
        tax_rate_pct: v.get("taxRatePct").num(),
        rate_shift_pct: v.get("rateShiftPct").num(),
    })
}

/// Writes JSON objects with keys in insertion order.
pub struct Out {
    s: String,
    first: bool,
}

impl Out {
    pub fn new() -> Out {
        Out {
            s: String::with_capacity(4096),
            first: true,
        }
    }

    pub fn finish(self) -> String {
        self.s
    }

    fn key(&mut self, k: &str) {
        if !self.first {
            self.s.push(',');
        }
        self.first = false;
        self.s.push('"');
        self.s.push_str(k);
        self.s.push_str("\":");
    }

    fn raw_num(&mut self, x: f64) {
        if x.is_finite() {
            let _ = write!(self.s, "{x}");
        } else if x.is_nan() {
            self.s.push_str("\"NaN\"");
        } else if x > 0.0 {
            self.s.push_str("\"Infinity\"");
        } else {
            self.s.push_str("\"-Infinity\"");
        }
    }

    pub fn open(&mut self, k: Option<&str>) {
        if let Some(k) = k {
            self.key(k);
        }
        self.s.push('{');
        self.first = true;
    }

    pub fn close(&mut self) {
        self.s.push('}');
        self.first = false;
    }

    pub fn num(&mut self, k: &str, x: f64) {
        self.key(k);
        self.raw_num(x);
    }

    pub fn opt_num(&mut self, k: &str, x: Option<f64>) {
        match x {
            Some(x) => self.num(k, x),
            None => self.null(k),
        }
    }

    pub fn nums(&mut self, k: &str, xs: &[f64]) {
        self.key(k);
        self.s.push('[');
        for (i, &x) in xs.iter().enumerate() {
            if i > 0 {
                self.s.push(',');
            }
            self.raw_num(x);
        }
        self.s.push(']');
    }

    /// `v` must not need escaping (codes only).
    pub fn code(&mut self, k: &str, v: &str) {
        self.key(k);
        self.s.push('"');
        self.s.push_str(v);
        self.s.push('"');
    }

    pub fn bool(&mut self, k: &str, v: bool) {
        self.key(k);
        self.s.push_str(if v { "true" } else { "false" });
    }

    pub fn null(&mut self, k: &str) {
        self.key(k);
        self.s.push_str("null");
    }

    fn open_array(&mut self, k: &str) {
        self.key(k);
        self.s.push('[');
        self.first = true;
    }

    fn close_array(&mut self) {
        self.s.push(']');
        self.first = false;
    }

    fn element(&mut self) {
        if !self.first {
            self.s.push(',');
        }
        self.first = false;
        self.s.push('{');
        self.first = true;
    }
}

fn schedule(o: &mut Out, k: &str, s: &Schedule) {
    o.open(Some(k));
    o.nums("days", &s.days);
    o.nums("coupons", &s.coupons);
    o.nums("principals", &s.principals);
    o.close();
}

/// `{"ok": derived}` or `{"error": code}`.
pub fn write_derived(r: &Result<Derived, Error>) -> String {
    let mut o = Out::new();
    o.open(None);
    match r {
        Err(e) => o.code("error", e.code()),
        Ok(d) => {
            o.open(Some("ok"));
            o.num("maturityDay", d.maturity_day);
            o.nums("couponDays", &d.coupon_days);
            o.nums("ratesPct", &d.rates_pct);
            o.nums("amortDays", &d.amort_days);
            o.nums("amortFracs", &d.amort_fracs);
            o.num("daysSinceLast", d.days_since_last);
            o.num("couponAmount", d.coupon_amount);
            o.num("accrued", d.accrued);
            o.num("dirtyPrice", d.dirty_price);
            schedule(&mut o, "flows", &d.flows);
            match &d.flows_to_offer {
                Some(s) => schedule(&mut o, "flowsToOffer", s),
                None => o.null("flowsToOffer"),
            }
            o.opt_num("offerDay", d.offer_day);
            o.num("ytmMaturity", d.ytm_maturity);
            o.opt_num("ytmOffer", d.ytm_offer);
            o.num("ytmSimple", d.ytm_simple);
            o.code("event", d.event.code());
            o.num("eventDay", d.event_day);
            o.num("yieldEvent", d.yield_event);
            o.num("macaulay", d.macaulay);
            o.num("modified", d.modified);
            o.close();
        }
    }
    o.close();
    o.finish()
}

fn breakdown_fields(o: &mut Out, b: &Breakdown) {
    o.num("qty", b.qty);
    o.num("invested", b.invested);
    o.num("coupons", b.coupons);
    o.num("reinvest", b.reinvest);
    o.num("amort", b.amort);
    o.num("body", b.body);
    o.num("tax", b.tax);
    o.num("commission", b.commission);
    o.num("total", b.total);
    o.num("profit", b.profit);
    o.num("annualPct", b.annual_pct);
    o.num("horizonDay", b.horizon_day);
}

fn breakdown(o: &mut Out, k: &str, b: &Breakdown) {
    o.open(Some(k));
    breakdown_fields(o, b);
    o.close();
}

/// `{"ok": calculation}` or `{"error": code}`.
pub fn write_calculation(r: &Result<Calculation, Error>) -> String {
    let mut o = Out::new();
    o.open(None);
    match r {
        Err(e) => o.code("error", e.code()),
        Ok(c) => {
            o.open(Some("ok"));
            breakdown(&mut o, "plan", &c.plan);
            o.open(Some("earlyExit"));
            o.num("rateShiftPct", c.early_exit.rate_shift_pct);
            o.bool("applicable", c.early_exit.applicable);
            breakdown(&mut o, "result", &c.early_exit.result);
            o.num("diff", c.early_exit.diff);
            o.num("modDurationAtHorizon", c.early_exit.mod_duration_at_horizon);
            o.close();
            match &c.floater {
                None => o.null("floater"),
                Some(f) => {
                    o.open(Some("floater"));
                    o.nums("days", &f.days);
                    o.open_array("scenarios");
                    for s in &f.scenarios {
                        o.element();
                        o.num("shiftPct", s.shift_pct);
                        breakdown(&mut o, "breakdown", &s.breakdown);
                        o.nums("coupons", &s.coupons);
                        o.close();
                    }
                    o.close_array();
                    o.close();
                }
            }
            match &c.offer {
                None => o.null("offer"),
                Some(p) => {
                    o.open(Some("offer"));
                    breakdown(&mut o, "before", &p.before);
                    breakdown(&mut o, "after", &p.after);
                    o.close();
                }
            }
            o.close();
        }
    }
    o.close();
    o.finish()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_json() {
        let v = parse(r#" {"a": [1, -2.5e3, true, null, "x\"\u00e9\ud83d\ude00"], "b": {}} "#)
            .expect("valid");
        assert_eq!(
            v.get("a"),
            &Value::Arr(vec![
                Value::Num(1.0),
                Value::Num(-2500.0),
                Value::Bool(true),
                Value::Null,
                Value::Str("x\"\u{e9}\u{1f600}".into()),
            ])
        );
        assert_eq!(v.get("b"), &Value::Obj(vec![]));
        assert_eq!(v.get("missing"), &Value::Null);
    }

    #[test]
    fn rejects_invalid_json() {
        for s in [
            "",
            "{",
            "[1,]",
            "{\"a\" 1}",
            "01x",
            "1.",
            ".5",
            "inf",
            "\"\\x\"",
            "[1] 2",
            "nul",
        ] {
            assert_eq!(parse(s), None, "{s}");
        }
        assert_eq!(parse(&"[".repeat(100)), None);
    }

    #[test]
    fn writes_special_numbers_as_strings() {
        let mut o = Out::new();
        o.open(None);
        o.nums(
            "x",
            &[1.5, f64::NAN, f64::INFINITY, f64::NEG_INFINITY, 1e21, 1e-7],
        );
        o.close();
        assert_eq!(
            o.finish(),
            r#"{"x":[1.5,"NaN","Infinity","-Infinity",1000000000000000000000,0.0000001]}"#
        );
    }
}

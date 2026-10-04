//! Checks the Rust implementation against every case in `cases.json`.
//!
//! Numbers compare within a relative tolerance of 1e-6: `|got - want| <=
//! 1e-6 * max(|got|, |want|)`, so an expected zero must be exactly zero.
//! NaN is written `"NaN"` in the table and must come out as NaN. Strings,
//! booleans and nulls compare exactly; objects must have the same keys.

use horkos_yield::*;
use serde_json::{json, Map, Value};

const CASES: &str = include_str!("../cases.json");
const RELATIVE: f64 = 1e-6;

fn num(v: &Value) -> f64 {
    match v {
        Value::String(s) if s == "NaN" => f64::NAN,
        _ => v.as_f64().expect("number"),
    }
}

fn arr(v: &Value) -> Vec<f64> {
    v.as_array().expect("array").iter().map(num).collect()
}

fn u32_arg(v: &Value) -> u32 {
    v.as_u64().expect("integer") as u32
}

fn text(v: &Value) -> String {
    v.as_str().expect("string").to_owned()
}

fn f(x: f64) -> Value {
    if x.is_nan() {
        json!("NaN")
    } else {
        json!(x)
    }
}

fn fs(xs: &[f64]) -> Value {
    Value::Array(xs.iter().copied().map(f).collect())
}

fn opt(x: Option<f64>) -> Value {
    x.map_or(Value::Null, f)
}

fn issue(v: &Value) -> Issue {
    Issue {
        nominal: num(&v["nominal"]),
        price_pct: num(&v["pricePct"]),
        accrued: if v["accrued"].is_null() {
            None
        } else {
            Some(num(&v["accrued"]))
        },
        coupon_type: CouponType::from_code(v["couponType"].as_str().expect("coupon type"))
            .expect("known coupon type"),
        coupon_rate_pct: num(&v["couponRatePct"]),
        spread_pct: num(&v["spreadPct"]),
        period_days: num(&v["periodDays"]),
        maturity: text(&v["maturity"]),
        offers: v["offers"]
            .as_array()
            .expect("offers")
            .iter()
            .map(text)
            .collect(),
        amortization: v["amortization"]
            .as_array()
            .expect("amortization")
            .iter()
            .map(|a| Amortization {
                date: text(&a["date"]),
                fraction_pct: num(&a["fractionPct"]),
            })
            .collect(),
    }
}

fn market(v: &Value) -> Market {
    Market {
        valuation_date: text(&v["valuationDate"]),
        key_rate_pct: num(&v["keyRatePct"]),
    }
}

fn plan(v: &Value) -> Plan {
    Plan {
        amount: num(&v["amount"]),
        horizon_day: num(&v["horizonDay"]),
        reinvest: v["reinvest"].as_bool().expect("reinvest"),
        tax_regime: TaxRegime::from_code(v["taxRegime"].as_str().expect("tax regime"))
            .expect("known tax regime"),
        tax_rate_pct: num(&v["taxRatePct"]),
        rate_shift_pct: num(&v["rateShiftPct"]),
    }
}

fn schedule(s: &Schedule) -> Value {
    json!({"days": fs(&s.days), "coupons": fs(&s.coupons), "principals": fs(&s.principals)})
}

fn derived(d: &Derived) -> Value {
    json!({
        "maturityDay": f(d.maturity_day),
        "couponDays": fs(&d.coupon_days),
        "ratesPct": fs(&d.rates_pct),
        "amortDays": fs(&d.amort_days),
        "amortFracs": fs(&d.amort_fracs),
        "daysSinceLast": f(d.days_since_last),
        "couponAmount": f(d.coupon_amount),
        "accrued": f(d.accrued),
        "dirtyPrice": f(d.dirty_price),
        "flows": schedule(&d.flows),
        "flowsToOffer": d.flows_to_offer.as_ref().map_or(Value::Null, schedule),
        "offerDay": opt(d.offer_day),
        "ytmMaturity": f(d.ytm_maturity),
        "ytmOffer": opt(d.ytm_offer),
        "ytmSimple": f(d.ytm_simple),
        "event": d.event.code(),
        "eventDay": f(d.event_day),
        "yieldEvent": f(d.yield_event),
        "macaulay": f(d.macaulay),
        "modified": f(d.modified),
    })
}

fn breakdown(b: &Breakdown) -> Value {
    json!({
        "qty": f(b.qty),
        "invested": f(b.invested),
        "coupons": f(b.coupons),
        "reinvest": f(b.reinvest),
        "amort": f(b.amort),
        "body": f(b.body),
        "tax": f(b.tax),
        "commission": f(b.commission),
        "total": f(b.total),
        "profit": f(b.profit),
        "annualPct": f(b.annual_pct),
        "horizonDay": f(b.horizon_day),
    })
}

fn calculation(c: &Calculation) -> Value {
    json!({
        "plan": breakdown(&c.plan),
        "earlyExit": {
            "rateShiftPct": f(c.early_exit.rate_shift_pct),
            "applicable": c.early_exit.applicable,
            "result": breakdown(&c.early_exit.result),
            "diff": f(c.early_exit.diff),
            "modDurationAtHorizon": f(c.early_exit.mod_duration_at_horizon),
        },
        "floater": c.floater.as_ref().map_or(Value::Null, |fl| json!({
            "days": fs(&fl.days),
            "scenarios": fl.scenarios.iter().map(|s| json!({
                "shiftPct": f(s.shift_pct),
                "breakdown": breakdown(&s.breakdown),
                "coupons": fs(&s.coupons),
            })).collect::<Vec<_>>(),
        })),
        "offer": c.offer.as_ref().map_or(Value::Null, |o| json!({
            "before": breakdown(&o.before),
            "after": breakdown(&o.after),
        })),
    })
}

fn outcome<T>(r: Result<T, Error>, ok: impl Fn(&T) -> Value) -> Value {
    let mut m = Map::new();
    match r {
        Ok(v) => m.insert("ok".into(), ok(&v)),
        Err(e) => m.insert("error".into(), json!(e.code())),
    };
    Value::Object(m)
}

fn run(name: &str, a: &[Value]) -> Value {
    match name {
        "price_from_yield" => f(price_from_yield(&arr(&a[0]), &arr(&a[1]), num(&a[2]))),
        "ytm_effective" => f(ytm_effective(&arr(&a[0]), &arr(&a[1]), num(&a[2]))),
        "ytm_simple" => f(ytm_simple(&arr(&a[0]), &arr(&a[1]), num(&a[2]))),
        "accrued_interest" => f(accrued_interest(num(&a[0]), num(&a[1]), num(&a[2]))),
        "macaulay_duration" => f(macaulay_duration(&arr(&a[0]), &arr(&a[1]), num(&a[2]))),
        "modified_duration" => f(modified_duration(&arr(&a[0]), &arr(&a[1]), num(&a[2]))),
        "build_cash_flow" => fs(&build_cash_flow(
            num(&a[0]),
            num(&a[1]),
            &arr(&a[2]),
            &arr(&a[3]),
            &arr(&a[4]),
            &arr(&a[5]),
            num(&a[6]),
            u32_arg(&a[7]),
            num(&a[8]),
        )),
        "floater_rate_path" => fs(&floater_rate_path(
            num(&a[0]),
            num(&a[1]),
            u32_arg(&a[2]),
            u32_arg(&a[3]),
        )),
        "floater_coupons" => fs(&floater_coupons(
            num(&a[0]),
            num(&a[1]),
            &arr(&a[2]),
            num(&a[3]),
        )),
        "tax_amount" => f(tax_amount(
            num(&a[0]),
            num(&a[1]),
            num(&a[2]),
            u32_arg(&a[3]),
            num(&a[4]),
        )),
        "hold_value" => fs(&hold_value(
            &arr(&a[0]),
            &arr(&a[1]),
            &arr(&a[2]),
            num(&a[3]),
            num(&a[4]),
            num(&a[5]),
        )),
        "price_after_rate_shift" => f(price_after_rate_shift(num(&a[0]), num(&a[1]), num(&a[2]))),
        "derive_bond" => outcome(derive_bond(&issue(&a[0]), &market(&a[1])), derived),
        "calculate" => outcome(
            calculate(&issue(&a[0]), &market(&a[1]), &plan(&a[2])),
            calculation,
        ),
        other => panic!("unknown function {other}"),
    }
}

fn close(g: f64, w: f64) -> bool {
    (g.is_nan() && w.is_nan()) || g == w || (g - w).abs() <= RELATIVE * g.abs().max(w.abs())
}

struct Report {
    failures: Vec<String>,
    worst: f64,
    worst_at: String,
}

fn compare(got: &Value, want: &Value, path: &str, r: &mut Report) {
    let is_num = |v: &Value| v.is_number() || v.as_str() == Some("NaN");
    match (got, want) {
        (g, w) if is_num(g) && is_num(w) => {
            let (g, w) = (num(g), num(w));
            if !close(g, w) {
                r.failures.push(format!("{path}: got {g}, want {w}"));
            } else if g != w && !g.is_nan() {
                let rel = (g - w).abs() / g.abs().max(w.abs());
                if rel > r.worst {
                    r.worst = rel;
                    r.worst_at = path.to_owned();
                }
            }
        }
        (Value::Array(g), Value::Array(w)) => {
            if g.len() != w.len() {
                r.failures
                    .push(format!("{path}: length {} vs {}", g.len(), w.len()));
                return;
            }
            for (i, (g, w)) in g.iter().zip(w).enumerate() {
                compare(g, w, &format!("{path}[{i}]"), r);
            }
        }
        (Value::Object(g), Value::Object(w)) => {
            let mut gk: Vec<_> = g.keys().collect();
            let mut wk: Vec<_> = w.keys().collect();
            gk.sort();
            wk.sort();
            if gk != wk {
                r.failures.push(format!("{path}: keys {gk:?} vs {wk:?}"));
                return;
            }
            for (k, wv) in w {
                compare(&g[k], wv, &format!("{path}.{k}"), r);
            }
        }
        (g, w) => {
            if g != w {
                r.failures.push(format!("{path}: got {g}, want {w}"));
            }
        }
    }
}

#[test]
fn every_case_matches() {
    let cases: Vec<Value> = serde_json::from_str(CASES).expect("valid cases.json");
    let mut r = Report {
        failures: Vec::new(),
        worst: 0.0,
        worst_at: String::new(),
    };
    let mut seen = std::collections::BTreeMap::new();
    for case in &cases {
        let name = case["name"].as_str().expect("case name");
        let func = case["fn"].as_str().expect("function name");
        *seen.entry(func).or_insert(0) += 1;
        let got = run(func, case["args"].as_array().expect("args array"));
        compare(&got, &case["expect"], name, &mut r);
    }
    assert!(
        r.failures.is_empty(),
        "failures:\n{}",
        r.failures.join("\n")
    );
    println!(
        "{} cases; worst relative difference {:e} at {}",
        cases.len(),
        r.worst,
        r.worst_at
    );
    // Every function has cases; the table keeps the 30 original primitive
    // cases at its head.
    assert_eq!(seen.len(), 14, "{seen:?}");
    assert!(cases.len() >= 30 + seen["derive_bond"] + seen["calculate"]);
}

#[test]
fn breakdown_lines_add_up() {
    let cases: Vec<Value> = serde_json::from_str(CASES).expect("valid cases.json");
    for case in cases.iter().filter(|c| c["fn"] == "calculate") {
        let a = case["args"].as_array().expect("args");
        let Ok(c) = calculate(&issue(&a[0]), &market(&a[1]), &plan(&a[2])) else {
            continue;
        };
        let b = c.plan;
        let sum = b.coupons + b.reinvest + b.amort + b.body + b.tax + b.commission;
        assert!((sum - b.total).abs() <= 1e-9 * b.total.abs().max(1.0));
        assert!(b.tax <= 0.0 && b.commission <= 0.0);
        assert!((b.profit - (b.total - b.invested)).abs() <= 1e-9 * b.total.abs().max(1.0));
    }
}

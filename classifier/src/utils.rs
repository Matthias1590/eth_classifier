use tch::{Kind, Tensor};

pub(crate) fn stats_from_intervals(intervals: Vec<u64>) -> (f64, f64, f64) {
    let t = Tensor::from_slice(&intervals.iter().map(|&x| x as f64).collect::<Vec<_>>());

    let mean = t.mean(Kind::Float).double_value(&[]);
    let std = t.std(false).double_value(&[]); // biased = population std
    let probs = &t / t.sum(Kind::Float);
    let entropy = (-(&probs * probs.log())).sum(Kind::Float).double_value(&[]);

    (mean, std, entropy)
}

pub(crate) fn stats_from_values(values: Vec<f64>) -> (f64, f64, f64, f64) {
    let t = Tensor::from_slice(&values);

    let mean = t.mean(Kind::Float).double_value(&[]);
    let median = t.median().double_value(&[]);
    let std = t.std(false).double_value(&[]);
    let max = t.max().double_value(&[]);

    (mean, median, std, max)
}

const EXCHANGES_TXT: &str = include_str!("../data/exchanges.txt");

pub(crate) fn is_exchange_owned(address: &str) -> bool {
    EXCHANGES_TXT.contains(&address.to_lowercase().as_str())
}

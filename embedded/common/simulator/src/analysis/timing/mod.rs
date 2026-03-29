use std::cmp::Ordering;
use std::collections::BTreeMap;
use std::ops::Add;
use std::ops::AddAssign;
use std::time::Duration;

use super::next_events::{AsyncTiming, NextEventsSpec, TraceStep};
use crate::{Event, OpId};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ElapsedTime {
    Exact(Duration),
    MoreThan(Duration),
}

impl ElapsedTime {
    fn zero() -> Self {
        Self::Exact(Duration::ZERO)
    }

    fn from_timing(timing: AsyncTiming) -> Self {
        match timing {
            AsyncTiming::Delay(duration) => Self::Exact(duration),
            AsyncTiming::Untimed => Self::MoreThan(Duration::ZERO),
        }
    }

    pub(crate) fn from_async_timing(timing: AsyncTiming) -> Self {
        Self::from_timing(timing)
    }
}

impl Ord for ElapsedTime {
    fn cmp(&self, other: &Self) -> Ordering {
        let (self_duration, self_more_than) = match self {
            ElapsedTime::Exact(duration) => (*duration, false),
            ElapsedTime::MoreThan(duration) => (*duration, true),
        };
        let (other_duration, other_more_than) = match other {
            ElapsedTime::Exact(duration) => (*duration, false),
            ElapsedTime::MoreThan(duration) => (*duration, true),
        };

        self_duration
            .cmp(&other_duration)
            .then_with(|| self_more_than.cmp(&other_more_than))
    }
}

impl PartialOrd for ElapsedTime {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Add for ElapsedTime {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        match (self, rhs) {
            (ElapsedTime::Exact(left), ElapsedTime::Exact(right)) => {
                ElapsedTime::Exact(left + right)
            }
            (ElapsedTime::Exact(left), ElapsedTime::MoreThan(right)) => {
                ElapsedTime::MoreThan(left + right)
            }
            (ElapsedTime::MoreThan(left), ElapsedTime::Exact(right)) => {
                ElapsedTime::MoreThan(left + right)
            }
            (ElapsedTime::MoreThan(left), ElapsedTime::MoreThan(right)) => {
                ElapsedTime::MoreThan(left + right)
            }
        }
    }
}

impl AddAssign for ElapsedTime {
    fn add_assign(&mut self, rhs: Self) {
        *self = *self + rhs;
    }
}

pub fn elapsed_time<S, A, SR, AR, Spec>(trace: &[TraceStep<S, A, SR, AR>]) -> ElapsedTime
where
    S: Clone,
    A: Clone,
    Spec: NextEventsSpec<S, A, SR, AR>,
{
    if trace.len() <= 1 {
        return ElapsedTime::zero();
    }

    let mut outbound_async_ops = BTreeMap::<OpId, A>::new();
    let mut elapsed = ElapsedTime::zero();

    for step in trace {
        for event in &step.outbound {
            if let Event::CreateAsync { id, op } = event {
                outbound_async_ops.insert(*id, op.clone());
            }
        }

        if let Some(Event::ResolveAsync { id, .. }) = &step.inbound {
            if let Some(op) = outbound_async_ops.get(id) {
                elapsed += ElapsedTime::from_timing(Spec::async_timing(op));
            }
        }
    }

    elapsed
}

#[cfg(test)]
mod tests;

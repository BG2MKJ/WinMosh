use std::collections::VecDeque;
use std::fmt;

use crate::proto::TransportInstruction;

pub const MAX_STATE_HISTORY: usize = 32;

pub trait StateObject: Clone + PartialEq {
    fn diff_from(&self, existing: &Self) -> Result<Vec<u8>, StateSyncError>;
    fn apply_diff(&mut self, diff: &[u8]) -> Result<(), StateSyncError>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StateSyncError {
    InvalidInstruction(&'static str),
    MissingBase(u64),
    StateNumberExhausted,
    InvalidDiff(String),
}

impl fmt::Display for StateSyncError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidInstruction(message) => {
                write!(formatter, "invalid SSP instruction: {message}")
            }
            Self::MissingBase(number) => {
                write!(formatter, "SSP base state is unavailable: {number}")
            }
            Self::StateNumberExhausted => formatter.write_str("SSP state number exhausted"),
            Self::InvalidDiff(message) => write!(formatter, "invalid SSP state diff: {message}"),
        }
    }
}

impl std::error::Error for StateSyncError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ByteState(pub Vec<u8>);

impl StateObject for ByteState {
    fn diff_from(&self, existing: &Self) -> Result<Vec<u8>, StateSyncError> {
        if self == existing {
            Ok(Vec::new())
        } else {
            Ok(self.0.clone())
        }
    }

    fn apply_diff(&mut self, diff: &[u8]) -> Result<(), StateSyncError> {
        self.0 = diff.to_vec();
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct StateSnapshot<S> {
    number: u64,
    state: S,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReceiveResult<S> {
    Applied { number: u64, state: S },
    Duplicate { number: u64, state: S },
    Shutdown,
}

pub struct StateSyncSender<S> {
    current_state: S,
    sent_states: VecDeque<StateSnapshot<S>>,
    next_number: u64,
    acknowledged_number: u64,
    remote_ack_number: u64,
    last_instruction: Option<TransportInstruction>,
    last_sent_at: Option<u64>,
}

impl<S> StateSyncSender<S>
where
    S: StateObject,
{
    pub fn new(initial_state: S) -> Self {
        Self {
            current_state: initial_state.clone(),
            sent_states: VecDeque::from([StateSnapshot {
                number: 0,
                state: initial_state,
            }]),
            next_number: 0,
            acknowledged_number: 0,
            remote_ack_number: 0,
            last_instruction: None,
            last_sent_at: None,
        }
    }

    pub fn set_state(&mut self, state: S) {
        self.current_state = state;
    }

    pub fn current_state(&self) -> &S {
        &self.current_state
    }

    pub fn last_sent_at(&self) -> Option<u64> {
        self.last_sent_at
    }

    pub fn build_instruction(
        &mut self,
        now_ms: u64,
    ) -> Result<TransportInstruction, StateSyncError> {
        let latest_number = self
            .sent_states
            .back()
            .map(|snapshot| snapshot.number)
            .ok_or(StateSyncError::InvalidInstruction("empty sender history"))?;
        let new_number = if self
            .sent_states
            .back()
            .map(|snapshot| snapshot.state != self.current_state)
            .unwrap_or(true)
        {
            if self.next_number >= u64::MAX - 1 {
                return Err(StateSyncError::StateNumberExhausted);
            }
            self.next_number += 1;
            self.sent_states.push_back(StateSnapshot {
                number: self.next_number,
                state: self.current_state.clone(),
            });
            self.next_number
        } else {
            latest_number
        };

        let base = self
            .sent_states
            .iter()
            .find(|snapshot| snapshot.number == self.acknowledged_number)
            .or_else(|| self.sent_states.front())
            .ok_or(StateSyncError::InvalidInstruction("empty sender history"))?;
        let diff = self.current_state.diff_from(&base.state)?;
        let instruction = TransportInstruction {
            protocol_version: Some(2),
            old_num: Some(base.number),
            new_num: Some(new_number),
            ack_num: Some(self.remote_ack_number),
            throwaway_num: self.sent_states.front().map(|snapshot| snapshot.number),
            diff: Some(diff),
            chaff: None,
        };
        self.last_instruction = Some(instruction.clone());
        self.last_sent_at = Some(now_ms);
        Ok(instruction)
    }

    pub fn retransmission(&self, now_ms: u64, timeout_ms: u64) -> Option<TransportInstruction> {
        self.last_sent_at
            .filter(|sent_at| now_ms.saturating_sub(*sent_at) >= timeout_ms)
            .and(self.last_instruction.clone())
    }

    pub fn acknowledge_remote(&mut self, ack_number: u64) {
        self.remote_ack_number = self.remote_ack_number.max(ack_number);
    }

    pub fn acknowledge_local(&mut self, ack_number: u64) {
        self.acknowledged_number = self.acknowledged_number.max(ack_number);
        while self.sent_states.len() > 1
            && self
                .sent_states
                .front()
                .map(|snapshot| snapshot.number < self.acknowledged_number)
                .unwrap_or(false)
        {
            self.sent_states.pop_front();
        }
        while self.sent_states.len() > MAX_STATE_HISTORY {
            self.sent_states.pop_front();
        }
    }

    pub fn history_len(&self) -> usize {
        self.sent_states.len()
    }
}

pub struct StateSyncReceiver<S> {
    received_states: VecDeque<StateSnapshot<S>>,
    last_ack_number: u64,
}

impl<S> StateSyncReceiver<S>
where
    S: StateObject,
{
    pub fn new(initial_state: S) -> Self {
        Self {
            received_states: VecDeque::from([StateSnapshot {
                number: 0,
                state: initial_state,
            }]),
            last_ack_number: 0,
        }
    }

    pub fn apply_instruction(
        &mut self,
        instruction: &TransportInstruction,
    ) -> Result<ReceiveResult<S>, StateSyncError> {
        let new_number = instruction
            .new_num
            .ok_or(StateSyncError::InvalidInstruction("missing new_num"))?;
        if new_number == u64::MAX {
            return Ok(ReceiveResult::Shutdown);
        }
        let old_number = instruction
            .old_num
            .ok_or(StateSyncError::InvalidInstruction("missing old_num"))?;
        if new_number <= self.last_ack_number {
            let state = self
                .latest_state()
                .ok_or(StateSyncError::InvalidInstruction("empty receiver history"))?
                .clone();
            return Ok(ReceiveResult::Duplicate {
                number: self.last_ack_number,
                state,
            });
        }

        let base = self
            .received_states
            .iter()
            .find(|snapshot| snapshot.number == old_number)
            .ok_or(StateSyncError::MissingBase(old_number))?;
        let mut state = base.state.clone();
        if let Some(diff) = &instruction.diff {
            if !diff.is_empty() {
                state.apply_diff(diff)?;
            }
        }
        self.received_states.push_back(StateSnapshot {
            number: new_number,
            state: state.clone(),
        });
        self.last_ack_number = new_number;

        if let Some(throwaway_number) = instruction.throwaway_num {
            while self.received_states.len() > 1
                && self
                    .received_states
                    .front()
                    .map(|snapshot| snapshot.number < throwaway_number)
                    .unwrap_or(false)
            {
                self.received_states.pop_front();
            }
        }
        while self.received_states.len() > MAX_STATE_HISTORY {
            self.received_states.pop_front();
        }

        Ok(ReceiveResult::Applied {
            number: new_number,
            state,
        })
    }

    pub fn ack_instruction(&self) -> TransportInstruction {
        TransportInstruction {
            protocol_version: Some(2),
            old_num: Some(self.last_ack_number),
            new_num: Some(self.last_ack_number),
            ack_num: Some(self.last_ack_number),
            throwaway_num: self.received_states.front().map(|snapshot| snapshot.number),
            diff: Some(Vec::new()),
            chaff: None,
        }
    }

    pub fn latest_number(&self) -> u64 {
        self.last_ack_number
    }

    pub fn latest_state(&self) -> Option<&S> {
        self.received_states.back().map(|snapshot| &snapshot.state)
    }

    pub fn history_len(&self) -> usize {
        self.received_states.len()
    }
}

#[cfg(test)]
mod tests {
    use super::{ByteState, ReceiveResult, StateSyncReceiver, StateSyncSender};

    #[test]
    fn sender_and_receiver_converge_and_ack() -> Result<(), Box<dyn std::error::Error>> {
        let mut sender = StateSyncSender::new(ByteState(Vec::new()));
        let mut receiver = StateSyncReceiver::new(ByteState(Vec::new()));
        sender.set_state(ByteState(b"one".to_vec()));
        let first = sender.build_instruction(0)?;
        let applied = receiver.apply_instruction(&first)?;
        assert!(matches!(applied, ReceiveResult::Applied { number: 1, .. }));
        sender.set_state(ByteState(b"two".to_vec()));
        let second = sender.build_instruction(10)?;
        let applied = receiver.apply_instruction(&second)?;
        assert!(matches!(applied, ReceiveResult::Applied { number: 2, .. }));
        sender.acknowledge_local(receiver.latest_number());
        assert_eq!(sender.history_len(), 1);
        assert_eq!(receiver.latest_state(), Some(&ByteState(b"two".to_vec())));
        Ok(())
    }

    #[test]
    fn retransmission_is_due_after_timeout() -> Result<(), Box<dyn std::error::Error>> {
        let mut sender = StateSyncSender::new(ByteState(Vec::new()));
        sender.build_instruction(100)?;
        assert!(sender.retransmission(149, 50).is_none());
        assert!(sender.retransmission(150, 50).is_some());
        Ok(())
    }

    #[test]
    fn receiver_rejects_missing_base() -> Result<(), Box<dyn std::error::Error>> {
        let mut receiver = StateSyncReceiver::new(ByteState(Vec::new()));
        let instruction = super::TransportInstruction {
            protocol_version: Some(2),
            old_num: Some(99),
            new_num: Some(100),
            ack_num: Some(0),
            throwaway_num: None,
            diff: Some(b"state".to_vec()),
            chaff: None,
        };
        assert!(receiver.apply_instruction(&instruction).is_err());
        Ok(())
    }

    #[test]
    fn rapid_state_changes_remain_consistent() -> Result<(), Box<dyn std::error::Error>> {
        let mut sender = StateSyncSender::new(ByteState(Vec::new()));
        let mut receiver = StateSyncReceiver::new(ByteState(Vec::new()));
        for i in 1..=100 {
            let value = format!("state_{i}");
            sender.set_state(ByteState(value.as_bytes().to_vec()));
            let instruction = sender.build_instruction(i * 10)?;
            let applied = receiver.apply_instruction(&instruction)?;
            assert!(matches!(applied, ReceiveResult::Applied { number: n, .. } if n == i));
            sender.acknowledge_local(receiver.latest_number());
        }
        assert_eq!(
            receiver.latest_state(),
            Some(&ByteState(b"state_100".to_vec()))
        );
        assert!(sender.history_len() <= 2);
        Ok(())
    }

    #[test]
    fn handles_missing_intermediate_states() -> Result<(), Box<dyn std::error::Error>> {
        let mut sender = StateSyncSender::new(ByteState(Vec::new()));
        let mut receiver = StateSyncReceiver::new(ByteState(Vec::new()));

        sender.set_state(ByteState(b"one".to_vec()));
        let first = sender.build_instruction(0)?;
        sender.set_state(ByteState(b"two".to_vec()));
        let _second = sender.build_instruction(10)?;
        sender.set_state(ByteState(b"three".to_vec()));
        let third = sender.build_instruction(20)?;

        receiver.apply_instruction(&first)?;
        let result = receiver.apply_instruction(&third)?;
        assert!(matches!(result, ReceiveResult::Applied { number: 3, .. }));
        assert_eq!(
            receiver.latest_state(),
            Some(&ByteState(b"three".to_vec()))
        );
        Ok(())
    }

    #[test]
    fn duplicate_state_is_ignored() -> Result<(), Box<dyn std::error::Error>> {
        let mut sender = StateSyncSender::new(ByteState(Vec::new()));
        let mut receiver = StateSyncReceiver::new(ByteState(Vec::new()));
        sender.set_state(ByteState(b"data".to_vec()));
        let instruction = sender.build_instruction(0)?;
        receiver.apply_instruction(&instruction)?;
        let result = receiver.apply_instruction(&instruction)?;
        assert!(matches!(result, ReceiveResult::Duplicate { .. }));
        Ok(())
    }

    #[test]
    fn receiver_trims_history_at_max_capacity() -> Result<(), Box<dyn std::error::Error>> {
        let mut sender = StateSyncSender::new(ByteState(Vec::new()));
        let mut receiver = StateSyncReceiver::new(ByteState(Vec::new()));
        for i in 1..=50 {
            let value = format!("state_{i}");
            sender.set_state(ByteState(value.as_bytes().to_vec()));
            let instruction = sender.build_instruction(i * 10)?;
            sender.acknowledge_local(receiver.latest_number());
            receiver.apply_instruction(&instruction)?;
        }
        assert!(receiver.history_len() <= 32);
        Ok(())
    }

    #[test]
    fn sender_does_not_exhaust_state_numbers() -> Result<(), Box<dyn std::error::Error>> {
        let mut sender = StateSyncSender::new(ByteState(Vec::new()));
        let mut state = Vec::new();
        for i in 0..1000 {
            state.push((i % 256) as u8);
            sender.set_state(ByteState(state.clone()));
            sender.build_instruction(i * 10)?;
        }
        Ok(())
    }

    #[test]
    fn empty_diff_when_state_unchanged() -> Result<(), Box<dyn std::error::Error>> {
        let mut sender = StateSyncSender::new(ByteState(b"same".to_vec()));
        sender.set_state(ByteState(b"same".to_vec()));
        let instruction = sender.build_instruction(0)?;
        assert!(instruction.diff.unwrap_or_default().is_empty());
        Ok(())
    }

    #[test]
    fn survives_random_packet_loss() -> Result<(), Box<dyn std::error::Error>> {
        let mut sender = StateSyncSender::new(ByteState(Vec::new()));
        let mut receiver = StateSyncReceiver::new(ByteState(Vec::new()));
        let mut rng = SimpleRng::new(42);
        let mut last_sent: Option<super::TransportInstruction> = None;
        let mut time = 0_u64;

        for i in 1..=200 {
            let bytes = format!("state_{i}").into_bytes();
            sender.set_state(ByteState(bytes));
            let instruction = sender.build_instruction(time)?;
            time += 10;

            if rng.chance(70) {
                receiver.apply_instruction(&instruction)?;
                sender.acknowledge_local(receiver.latest_number());
            }
            last_sent = Some(instruction);

            if rng.chance(30) {
                if let Some(ref instr) = last_sent {
                    if rng.chance(50) {
                        receiver.apply_instruction(instr)?;
                        sender.acknowledge_local(receiver.latest_number());
                    }
                }
            }
        }

        while sender.history_len() > 1 {
            let instr = sender.build_instruction(time)?;
            time += 10;
            receiver.apply_instruction(&instr)?;
            sender.acknowledge_local(receiver.latest_number());
        }

        assert_eq!(
            receiver.latest_state(),
            Some(&ByteState(b"state_200".to_vec()))
        );
        Ok(())
    }

    #[test]
    fn recovers_after_long_disconnect() -> Result<(), Box<dyn std::error::Error>> {
        let mut sender = StateSyncSender::new(ByteState(Vec::new()));
        let mut receiver = StateSyncReceiver::new(ByteState(Vec::new()));
        let mut time = 0_u64;

        for i in 1..=10 {
            sender.set_state(ByteState(format!("pre_{i}").into_bytes()));
            let instr = sender.build_instruction(time)?;
            time += 10;
            receiver.apply_instruction(&instr)?;
            sender.acknowledge_local(receiver.latest_number());
        }

        for i in 11..=50 {
            sender.set_state(ByteState(format!("lost_{i}").into_bytes()));
            sender.build_instruction(time)?;
            time += 1000;
        }

        let final_state = ByteState(b"recovered".to_vec());
        sender.set_state(final_state.clone());
        let instr = sender.build_instruction(time)?;
        receiver.apply_instruction(&instr)?;
        sender.acknowledge_local(receiver.latest_number());

        assert_eq!(receiver.latest_state(), Some(&final_state));
        Ok(())
    }

    #[test]
    fn handles_out_of_order_delivery() -> Result<(), Box<dyn std::error::Error>> {
        let mut sender = StateSyncSender::new(ByteState(Vec::new()));
        let mut receiver = StateSyncReceiver::new(ByteState(Vec::new()));
        let mut time = 0_u64;
        let mut instructions = Vec::new();

        for i in 1..=10 {
            sender.set_state(ByteState(format!("msg_{i}").into_bytes()));
            instructions.push(sender.build_instruction(time)?);
            time += 10;
        }

        let order = [3, 1, 5, 2, 7, 4, 9, 6, 10, 8];
        for &idx in &order {
            receiver.apply_instruction(&instructions[idx - 1])?;
            sender.acknowledge_local(receiver.latest_number());
        }

        let expected = ByteState(b"msg_10".to_vec());
        assert_eq!(receiver.latest_state(), Some(&expected));
        Ok(())
    }

    #[test]
    fn retransmission_fills_gaps() -> Result<(), Box<dyn std::error::Error>> {
        let mut sender = StateSyncSender::new(ByteState(Vec::new()));
        let mut receiver = StateSyncReceiver::new(ByteState(Vec::new()));
        let mut time = 0_u64;

        sender.set_state(ByteState(b"first".to_vec()));
        let first = sender.build_instruction(time)?;
        time += 10;
        receiver.apply_instruction(&first)?;
        sender.acknowledge_local(receiver.latest_number());

        sender.set_state(ByteState(b"second".to_vec()));
        let _lost = sender.build_instruction(time)?;
        time += 10;

        sender.set_state(ByteState(b"third".to_vec()));
        let _lost2 = sender.build_instruction(time)?;
        time += 10;

        sender.set_state(ByteState(b"fourth".to_vec()));
        let fourth = sender.build_instruction(time)?;

        receiver.apply_instruction(&fourth)?;
        sender.acknowledge_local(receiver.latest_number());

        assert_eq!(
            receiver.latest_state(),
            Some(&ByteState(b"fourth".to_vec()))
        );
        Ok(())
    }

    struct SimpleRng {
        state: u64,
    }

    impl SimpleRng {
        fn new(seed: u64) -> Self {
            Self { state: seed }
        }

        fn next(&mut self) -> u64 {
            self.state = self.state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            (self.state >> 33) as u64
        }

        fn chance(&mut self, percent: u8) -> bool {
            (self.next() % 100) < u64::from(percent)
        }
    }
}
